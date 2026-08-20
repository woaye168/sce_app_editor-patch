//! 「应用补丁」流程：预算 → 整库备份（仅首次）→ 整库解密 → 应用插槽（三级回退）→ 补丁目录/默认模块。

use super::{
    find_reusable_version, new_progress, pie_capture_slot_warning, set_phase, set_total, LibSpec,
    SharedProgress, LIBS,
};
use crate::core::locate::EditorTarget;
use crate::core::{backup, crypto, locate, log, modules, ops, slot_inject, slots};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// 单库应用作业（预算阶段收集）
struct Job<'a> {
    lib: &'a LibSpec,
    dir: PathBuf,
    group: String,
    version: u64,
    needs_backup: bool,
    backup_count: usize,
    lua_files: Vec<PathBuf>,
    has_slots: bool,
}

/// 后台线程执行「应用补丁」，返回共享进度句柄（UI 轮询）
pub fn apply_async(project_root: PathBuf) -> SharedProgress {
    let progress = new_progress();
    let p = Arc::clone(&progress);
    std::thread::spawn(move || {
        let result = apply_all(&project_root, &p);
        let mut g = p.lock().unwrap();
        match result {
            Ok(summary) => {
                g.ok = true;
                g.summary = summary;
            }
            Err(e) => {
                g.ok = false;
                g.summary = e;
            }
        }
        g.finished = true;
    });
    progress
}

pub(crate) fn apply_all(project_root: &Path, progress: &SharedProgress) -> Result<String, String> {
    set_phase(progress, "定位编辑器…");
    let target = locate::locate(project_root)?;
    log::log(Some(project_root), Some(&target.editor_root), "INFO", "开始应用补丁");

    // 预算：收集每个库的文件清单，计算总进度
    let mut jobs = Vec::new();
    let mut pre_errors: Vec<String> = Vec::new();
    for lib in LIBS {
        match (|| {
            let dir = lib.package_dir(&target)?;
            let group = lib.backup_group(&target)?;
            let version = target.package_version(lib.pkg)?;
            let needs_backup = !backup::has_backup(&target.editor_root, &group);
            let backup_count = if needs_backup {
                ops::collect_all_files(&dir).len()
            } else {
                0
            };
            let lua_files = ops::collect_lua_files(&dir);
            let has_slots = slots::has_slots(lib.pkg, version);
            Ok::<_, String>(Job { lib, dir, group, version, needs_backup, backup_count, lua_files, has_slots })
        })() {
            Ok(job) => jobs.push(job),
            Err(e) => pre_errors.push(format!("✘ {}：{e}", lib.name)),
        }
    }
    if jobs.is_empty() {
        return Err(format!("没有可用的目标库：\n{}", pre_errors.join("\n")));
    }
    let total: usize = jobs.iter().map(|j| j.backup_count + j.lua_files.len()).sum();
    set_total(progress, total);
    let done = progress.lock().unwrap().done.clone();

    let version_dir = target.version_dir()?;
    let mut lines = pre_errors;
    let mut ok_count = 0;
    for job in &jobs {
        match apply_lib(&target, job, progress, &done, project_root, &version_dir) {
            Ok(msg) => {
                ok_count += 1;
                lines.push(format!("✔ {}：{msg}", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "INFO",
                    &format!("库[{}]应用成功: {msg}", job.lib.pkg));
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "ERROR",
                    &format!("库[{}]应用失败: {e}", job.lib.pkg));
            }
        }
    }
    if ok_count == 0 {
        return Err(format!("全部库应用失败：\n{}", lines.join("\n")));
    }
    Ok(lines.join("\n"))
}

/// 处理单个库：备份 → 整库解密 → 应用插槽（三级回退）→ 补丁目录/默认模块
fn apply_lib(
    target: &EditorTarget,
    job: &Job,
    progress: &SharedProgress,
    done: &Arc<AtomicUsize>,
    project_root: &Path,
    version_dir: &Path,
) -> Result<String, String> {
    let lib = job.lib;
    let mut parts: Vec<String> = Vec::new();

    // 1. 整库备份（仅首次）
    if job.needs_backup {
        set_phase(progress, format!("备份 {} 库…", lib.pkg));
        backup::backup_lib(&target.editor_root, &job.group, &job.dir, done)?;
        parts.push(format!("已备份 {} 个文件", job.backup_count));
    } else {
        parts.push("沿用已有备份".to_string());
    }

    // 2. 整库解密（明文文件自动跳过）
    set_phase(progress, format!("解密 {} 库…", lib.pkg));
    let decrypted = Arc::new(AtomicUsize::new(0));
    let d = Arc::clone(&decrypted);
    let errors = ops::parallel_for_each(&job.lua_files, done, move |file| {
        if ops::decrypt_file_in_place(file)? {
            d.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    });
    if !errors.is_empty() {
        return Err(format!(
            "解密失败 {} 个文件，已中止（首个错误：{}）",
            errors.len(),
            errors[0]
        ));
    }
    parts.push(format!("解密 {} 个文件", decrypted.load(Ordering::Relaxed)));

    // 3. 应用插槽（0.5.3 F4 三级回退：精确复制 → 同内容复用 → 运行时注入）
    set_phase(progress, format!("写入 {} 库插槽…", lib.pkg));
    if job.has_slots {
        let n = slots::apply_slots(lib.pkg, job.version, &job.dir)?;
        parts.push(format!("插槽文件 {n} 个"));
    } else if let Some(v) = find_reusable_version(lib.pkg, &job.dir, job.version) {
        let n = slots::apply_slots(lib.pkg, v, &job.dir)?;
        parts.push(format!("插槽复用自 v{v}（官方源内容一致，{n} 个文件）"));
    } else {
        let n = runtime_inject_slots(lib, &job.dir, job.version)?;
        parts.push(format!(
            "运行时注入插槽 {n} 个文件（建议重跑 make_slots 固化 v{} slots）",
            job.version
        ));
    }

    // 4. 补丁目录：首次创建启用默认模块，否则按现状重建入口
    let root = lib.require_root_dir(target)?;
    if modules::patch_dir(&root).exists() {
        modules::regenerate_entry(&root)?;
        parts.push("保留已启用模块".to_string());
    } else {
        let (defaults, warnings) = modules::apply_defaults(
            &root,
            lib.pkg,
            Some(version_dir),
            Some(project_root),
        )?;
        if defaults.is_empty() {
            parts.push("补丁目录已创建".to_string());
        } else {
            parts.push(format!("默认启用模块: {}", defaults.join(", ")));
        }
        parts.extend(warnings);
    }

    // pie_capture 行为插槽缺失时明确提示（兜底：不允许启用后静默不生效）
    if lib.pkg == "xdeditor"
        && modules::enabled_modules(&root).iter().any(|id| id == "pie_capture")
    {
        if let Some(w) = pie_capture_slot_warning(target) {
            parts.push(w);
        }
    }

    Ok(parts.join("，"))
}

/// 三级回退第 3 级：运行时模式注入（对新版本解码源现场执行插槽/解锁变换）。
/// 注入后校验标记在位；锚点匹配失败明确报错提示重跑 make_slots 人工固化。
/// 返回注入文件数。
fn runtime_inject_slots(lib: &LibSpec, pkg_dir: &Path, version: u64) -> Result<usize, String> {
    let fail = |reason: &str| {
        format!(
            "{} v{version} 源码结构变化，自动注入失败（{reason}），请重跑 make_slots 人工固化 slots",
            lib.pkg
        )
    };
    let mut count = 0;

    // 入口插槽：顶层 return 之前注入插槽块（先剥离旧插槽，幂等）
    let entry = pkg_dir.join(lib.entry);
    let raw = std::fs::read(&entry).map_err(|e| fail(&format!("读取入口失败: {e}")))?;
    let text = slot_inject::strip_slot(&slot_inject::decode_source(&raw));
    let injected = slot_inject::insert_slot(&text, &slot_inject::slot_block());
    if !injected.contains(slot_inject::INJECT_BEGIN) {
        return Err(fail("插槽块注入后标记缺失"));
    }
    if injected == text {
        return Err(fail("入口注入未产生变化"));
    }
    crypto::write_atomic(&entry, injected.as_bytes()).map_err(|e| fail(&format!("写入入口失败: {e}")))?;
    count += 1;

    // script 库追加 isolation.lua 解锁变换
    if lib.pkg == "script" {
        let iso = pkg_dir.join("common/isolation.lua");
        let raw = std::fs::read(&iso).map_err(|e| fail(&format!("读取 isolation.lua 失败: {e}")))?;
        let text = slot_inject::strip_unlock(&slot_inject::decode_source(&raw));
        let (out, n) = slot_inject::transform_unlock(&text);
        if n == 0 || !out.contains(slot_inject::UNLOCK_MARK) {
            return Err(fail("isolation.lua 未找到可解锁行"));
        }
        crypto::write_atomic(&iso, out.as_bytes()).map_err(|e| fail(&format!("写入 isolation.lua 失败: {e}")))?;
        count += 1;
    }

    Ok(count)
}
