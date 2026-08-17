//! 内核补丁：按库处理（0.3.1 复制式插槽架构）。
//!
//! 对每个目标库（`LIBS` 登记表）执行：
//! 1. **整库备份**（仅首次，见 backup 模块）
//! 2. **整库解密**：库内加密的 .lua 原地替换为明文源码（明文跳过）。多线程并行 + 进度上报。
//! 3. **应用插槽文件**：把仓库内嵌的 `slots/<库>/<版本>/` 整树复制覆盖进库目录。
//!    插槽文件是「完整新源码」（官方源码 + 插槽/修改），字节级复制，不做编解码。
//!    无匹配版本子目录则跳过并明确提示（不蛮干）。
//! 4. **补丁目录**：在库 require 根下创建 sce_app_editor-patch/，首次创建启用默认勾选模块
//!
//! 「还原补丁」：有备份的库整树覆盖还原 + 删除补丁目录。
//! 「状态检查」：插槽标记（+script 库解锁标记）是否在，编辑器升级覆盖后显示「未应用」。

use super::locate::EditorTarget;
use super::{backup, bridge_deploy, crypto, locate, log, modules, ops};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 插槽标记（插槽文件内含此注释即视为已应用）
pub const INJECT_MARK: &str = "-->> sce_app_editor-patch >>";
/// 解锁行标记前缀（script 库 isolation.lua 状态校验用）
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 一个目标库（补丁打在该库入口点）
pub struct LibSpec {
    /// 包名（api_pak_version.json 中的键）
    pub pkg: &'static str,
    /// 界面显示名
    pub name: &'static str,
    /// 库内 require 根（相对包目录）：package.path 指向的目录
    pub require_root: &'static str,
    /// 入口文件（相对包目录）：状态检查读取此文件确认插槽
    pub entry: &'static str,
}

/// 全部目标库（新增库补丁在此登记；插槽文件在 slots/<pkg>/<版本>/ 下用 include_dir 内嵌）
pub const LIBS: &[LibSpec] = &[
    LibSpec {
        pkg: "script",
        name: "script（游戏脚本/common 包）",
        require_root: "common",
        entry: "common/init.lua",
    },
    LibSpec {
        pkg: "xdeditor",
        name: "xdeditor（编辑器界面）",
        require_root: "",
        entry: "main.lua",
    },
];

impl LibSpec {
    pub fn package_dir(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        target.package_dir(self.pkg)
    }

    /// 库 require 根目录（补丁目录所在处）
    pub fn require_root_dir(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        Ok(self.package_dir(target)?.join(self.require_root))
    }

    fn entry_path(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        Ok(self.package_dir(target)?.join(self.entry))
    }

    fn backup_group(&self, target: &EditorTarget) -> Result<String, String> {
        target.backup_group(self.pkg)
    }
}

/// 单个库的状态
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum LibStatus {
    /// 已应用（入口插槽在位，script 库还要求 isolation 解锁标记在位）
    Applied,
    /// 未应用（原始状态，或被编辑器升级覆盖）
    NotApplied,
    /// 包目录/入口文件缺失
    Missing,
}

/// 单个库的完整状态（供 UI 展示）
pub struct LibStatusInfo {
    pub pkg: &'static str,
    pub label: &'static str,
    pub version: String,
    pub status: LibStatus,
    pub has_backup: bool,
    /// slots 目录是否有该版本的插槽文件
    pub has_slots: bool,
    pub path: String,
}

/// 检查全部目标库状态
pub fn check(target: &EditorTarget) -> Vec<LibStatusInfo> {
    LIBS.iter()
        .map(|lib| {
            let version = target.package_version(lib.pkg).ok();
            let version_str = version.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string());
            let has_backup = lib
                .backup_group(target)
                .map(|g| backup::has_backup(&target.editor_root, &g))
                .unwrap_or(false);
            let has_slots = version
                .map(|v| slots::has_slots(lib.pkg, v))
                .unwrap_or(false);
            let (path, status) = match lib.entry_path(target) {
                Ok(entry) => {
                    // read_lua 自适应加密/明文：未打补丁的加密入口也能正确读出
                    let s = match crypto::read_lua(&entry) {
                        Ok(text) if text.contains(INJECT_MARK) => {
                            if lib.pkg == "script" {
                                match lib.package_dir(target).map(|d| d.join("common/isolation.lua")) {
                                    Ok(iso) => match crypto::read_lua(&iso) {
                                        Ok(t) if t.contains(UNLOCK_MARK) => LibStatus::Applied,
                                        Ok(_) => LibStatus::NotApplied,
                                        Err(_) => LibStatus::Missing,
                                    },
                                    Err(_) => LibStatus::Missing,
                                }
                            } else {
                                LibStatus::Applied
                            }
                        }
                        Ok(_) => LibStatus::NotApplied,
                        Err(_) => LibStatus::Missing,
                    };
                    (entry.display().to_string(), s)
                }
                Err(e) => (e, LibStatus::Missing),
            };
            LibStatusInfo {
                pkg: lib.pkg,
                label: lib.name,
                version: version_str,
                status,
                has_backup,
                has_slots,
                path,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- 进度

/// 后台任务进度（应用/还原共用）
pub struct TaskProgress {
    pub phase: String,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub finished: bool,
    pub ok: bool,
    pub summary: String,
}

pub type SharedProgress = Arc<Mutex<TaskProgress>>;

fn new_progress() -> SharedProgress {
    Arc::new(Mutex::new(TaskProgress {
        phase: "准备中…".to_string(),
        total: 0,
        done: Arc::new(AtomicUsize::new(0)),
        finished: false,
        ok: false,
        summary: String::new(),
    }))
}

fn set_phase(progress: &SharedProgress, phase: impl Into<String>) {
    progress.lock().unwrap().phase = phase.into();
}

fn set_total(progress: &SharedProgress, total: usize) {
    progress.lock().unwrap().total = total;
}

// ---------------------------------------------------------------- 应用

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

fn apply_all(project_root: &Path, progress: &SharedProgress) -> Result<String, String> {
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

    let mut lines = pre_errors;
    let mut ok_count = 0;
    for job in &jobs {
        match apply_lib(&target, job, progress, &done) {
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

/// 处理单个库：备份 → 整库解密 → 应用插槽文件 → 补丁目录/默认模块
fn apply_lib(
    target: &EditorTarget,
    job: &Job,
    progress: &SharedProgress,
    done: &Arc<AtomicUsize>,
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

    // 3. 应用插槽文件（整树字节级覆盖，不做编解码）
    if job.has_slots {
        set_phase(progress, format!("写入 {} 库插槽…", lib.pkg));
        let n = slots::apply_slots(lib.pkg, job.version, &job.dir)?;
        parts.push(format!("插槽文件 {n} 个"));
    } else {
        parts.push(format!("无 v{} 插槽（跳过）", job.version));
    }

    // 4. 补丁目录：首次创建启用默认模块，否则按现状重建入口
    let root = lib.require_root_dir(target)?;
    if modules::patch_dir(&root).exists() {
        modules::regenerate_entry(&root)?;
        parts.push("保留已启用模块".to_string());
    } else {
        let defaults = modules::apply_defaults(&root, lib.pkg)?;
        if defaults.is_empty() {
            parts.push("补丁目录已创建".to_string());
        } else {
            parts.push(format!("默认启用模块: {}", defaults.join(", ")));
        }
    }

    Ok(parts.join("，"))
}

// ---------------------------------------------------------------- 还原

/// 后台线程执行「还原补丁」
pub fn restore_async(project_root: PathBuf) -> SharedProgress {
    let progress = new_progress();
    let p = Arc::clone(&progress);
    std::thread::spawn(move || {
        let result = restore_all(&project_root, &p);
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

fn restore_all(project_root: &Path, progress: &SharedProgress) -> Result<String, String> {
    set_phase(progress, "定位编辑器…");
    let target = locate::locate(project_root)?;
    log::log(Some(project_root), Some(&target.editor_root), "INFO", "开始还原补丁");

    struct RJob<'a> {
        lib: &'a LibSpec,
        dir: PathBuf,
        group: String,
        file_count: usize,
    }
    let mut jobs = Vec::new();
    for lib in LIBS {
        let (Ok(dir), Ok(group)) = (lib.package_dir(&target), lib.backup_group(&target)) else {
            continue;
        };
        if !backup::has_backup(&target.editor_root, &group) {
            continue;
        }
        let count = ops::collect_all_files(&dir).len();
        jobs.push(RJob { lib, dir, group, file_count: count });
    }
    if jobs.is_empty() {
        return Err("没有任何备份可用于还原（尚未应用过补丁）".to_string());
    }
    set_total(progress, jobs.iter().map(|j| j.file_count).sum());
    let done = progress.lock().unwrap().done.clone();

    let mut lines: Vec<String> = Vec::new();
    for job in &jobs {
        set_phase(progress, format!("还原 {} 库…", job.lib.pkg));
        match backup::restore_lib(&target.editor_root, &job.group, &job.dir, &done) {
            Ok(()) => {
                if let Ok(root) = job.lib.require_root_dir(&target) {
                    let patch_dir = modules::patch_dir(&root);
                    if patch_dir.exists() {
                        std::fs::remove_dir_all(&patch_dir)
                            .map_err(|e| format!("删除 {} 失败: {e}", patch_dir.display()))?;
                    }
                }
                lines.push(format!("✔ {}：已用备份整库还原", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "INFO",
                    &format!("库[{}]还原成功", job.lib.pkg));
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "ERROR",
                    &format!("库[{}]还原失败: {e}", job.lib.pkg));
            }
        }
    }

    // 追加恢复 sce.deps.json（若曾部署 MCP 桥）：失败只记日志，不中断整体还原
    let version_dir = target.version_dir();
    match bridge_deploy::restore_deps(&version_dir) {
        Ok(()) => {
            log::log(Some(project_root), Some(&target.editor_root), "INFO",
                "sce.deps.json 恢复检查完成");
        }
        Err(e) => {
            lines.push(format!("✘ 恢复 sce.deps.json：{e}"));
            log::log(Some(project_root), Some(&target.editor_root), "ERROR",
                &format!("恢复 sce.deps.json 失败: {e}"));
        }
    }
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------- 插槽文件（编译期内嵌仓库 slots/ 目录）

mod slots {
    use include_dir::{include_dir, Dir};
    use std::path::Path;

    static SLOTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/slots");

    /// 定位 slots/<pkg>/<version> 子目录。
    /// include_dir 的 Dir::get_dir 只做单层名称匹配，嵌套子目录（如 script/199/common）
    /// 的 path 带分隔符、get_dir 取不到，所以逐层下钻 + 名称比对。
    fn slot_dir(pkg: &str, version: u64) -> Option<&'static Dir<'static>> {
        let ver = version.to_string();
        SLOTS.dirs()
            .find(|d| d.path().file_name().map(|n| n == pkg).unwrap_or(false))?
            .dirs()
            .find(|d| d.path().file_name().map(|n| n == ver.as_str()).unwrap_or(false))
    }

    /// 是否有指定库/版本的插槽文件
    pub fn has_slots(pkg: &str, version: u64) -> bool {
        slot_dir(pkg, version).is_some()
    }

    /// 把 slots/<pkg>/<version>/ 整树覆盖写入包目录，返回写入文件数
    pub fn apply_slots(pkg: &str, version: u64, pkg_dir: &Path) -> Result<usize, String> {
        let dir = slot_dir(pkg, version)
            .ok_or_else(|| format!("无 {pkg} v{version} 的插槽文件"))?;
        let mut count = 0;
        write_dir(dir, pkg_dir, &mut count)?;
        Ok(count)
    }

    /// file.path() 相对 slots 根（含 <pkg>/<version>/ 前缀），写入时剥掉前缀
    fn write_dir(dir: &Dir, dest: &Path, count: &mut usize) -> Result<(), String> {
        for file in dir.files() {
            let rel = file.path();
            // 剥掉前两层（pkg/version）
            let rel = rel.iter().skip(2).collect::<std::path::PathBuf>();
            let target = dest.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
            }
            super::crypto::write_atomic(&target, file.contents())?;
            *count += 1;
        }
        for sub in dir.dirs() {
            write_dir(sub, dest, count)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slots_embedded() {
        // 编译期内嵌的 slots 文件可用（当前版本 199/160 必须存在）
        assert!(slots::has_slots("script", 199));
        assert!(slots::has_slots("xdeditor", 160));
        assert!(!slots::has_slots("script", 999));
    }

    /// 端到端：临时目录双库（加密+明文混合）整库流程
    #[test]
    fn test_apply_restore_flow() {
        let base = std::env::temp_dir().join(format!("editor_patch_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let backup_dir = base.join("backup");
        std::env::set_var("EDITOR_PATCH_BACKUP_DIR", &backup_dir);

        // 项目结构
        let project = base.join("project_x");
        std::fs::create_dir_all(project.join("project")).unwrap();
        std::fs::create_dir_all(project.join("script")).unwrap();
        std::fs::write(
            project.join("project").join("map_settings.json"),
            r#"{"api_version": {"api_version": 13}}"#,
        )
        .unwrap();
        let editor_root = base.join("editor");
        std::fs::write(
            project.join("script").join("tsconfig.json"),
            format!(
                r#"{{"compilerOptions": {{"typeRoots": ["{}"]}}}}"#,
                editor_root.display().to_string().replace('\\', "/")
                    + "/Res/_m/maps/global_default/53/global_default/script/"
            ),
        )
        .unwrap();
        std::fs::create_dir_all(&editor_root).unwrap();
        std::fs::write(
            editor_root.join("api_pak_version.json"),
            r##"{"#package_path": {"script": "Res/_m/script", "xdeditor": "Res/_m/xdeditor"},
                "13": {"script": 199, "xdeditor": 160}}"##,
        )
        .unwrap();

        // script 包：加密 init.lua / isolation.lua + 一个明文文件
        // 注意：测试里 api_pak 指 v199，而 slots/script/199/ 内嵌文件会在应用时覆盖 init/isolation
        let common = editor_root.join("Res/_m/script/199/script/common");
        std::fs::create_dir_all(&common).unwrap();
        let iso_original = "-- original isolation\n";
        let iso = common.join("isolation.lua");
        std::fs::write(&iso, crypto::encrypt(iso_original.as_bytes())).unwrap();
        let iso_original_bytes = std::fs::read(&iso).unwrap();
        let init = common.join("init.lua");
        std::fs::write(&init, crypto::encrypt(b"-- original init\n")).unwrap();
        let init_original_bytes = std::fs::read(&init).unwrap();
        let plain_file = common.join("plain_note.lua");
        std::fs::write(&plain_file, "-- 本来就是明文\n").unwrap();

        // xdeditor 包：加密 main.lua
        let xd = editor_root.join("Res/_m/xdeditor/160/xdeditor");
        std::fs::create_dir_all(&xd).unwrap();
        let xd_main = xd.join("main.lua");
        let xd_main_original_bytes = crypto::encrypt(b"-- original xdeditor main\n");
        std::fs::write(&xd_main, &xd_main_original_bytes).unwrap();

        // 应用
        let progress = new_progress();
        let msg = apply_all(&project, &progress).unwrap();
        assert!(msg.contains("插槽文件 2 个"), "{msg}"); // script: init+isolation；xdeditor: main

        // 状态：两库均已应用（插槽文件已覆盖入口/isolation）
        let target = locate::locate(&project).unwrap();
        let statuses = check(&target);
        for s in &statuses {
            assert_eq!(s.status, LibStatus::Applied, "[{}] status={:?} path={}", s.pkg, s.status, s.path);
        }

        // 插槽文件已生效（入口含插槽标记、isolation 含解锁标记），且为明文
        let init_text = std::fs::read_to_string(&init).unwrap();
        assert!(init_text.contains(INJECT_MARK));
        let iso_text = std::fs::read_to_string(&iso).unwrap();
        assert!(iso_text.contains(UNLOCK_MARK));
        // 明文文件未被破坏
        assert_eq!(std::fs::read_to_string(&plain_file).unwrap(), "-- 本来就是明文\n");

        // 默认模块已启用（xdeditor menu_bgd 明文写入）
        let menu_module = modules::patch_dir(&xd).join("menu_bgd").join("main.lua");
        assert!(menu_module.is_file());
        assert!(std::fs::read_to_string(&menu_module).unwrap().contains("window_title_bar_register"));

        // 再应用：幂等（备份不重复、模块保留）
        let progress2 = new_progress();
        let msg2 = apply_all(&project, &progress2).unwrap();
        assert!(msg2.contains("沿用已有备份"), "{msg2}");
        assert!(msg2.contains("保留已启用模块"), "{msg2}");

        // 模拟编辑器升级覆盖：入口换成全新加密原始文件 → 检测为未应用
        std::fs::write(&init, crypto::encrypt(b"-- original init\n")).unwrap();
        let target = locate::locate(&project).unwrap();
        let statuses = check(&target);
        let script_status = statuses.iter().find(|s| s.pkg == "script").unwrap();
        assert_eq!(script_status.status, LibStatus::NotApplied, "覆盖后应检测为未应用");

        // 还原：整库字节级还原（含被插槽覆盖的文件回到加密原始字节）
        let progress3 = new_progress();
        restore_all(&project, &progress3).unwrap();
        assert_eq!(std::fs::read(&iso).unwrap(), iso_original_bytes);
        assert_eq!(std::fs::read(&init).unwrap(), init_original_bytes);
        assert_eq!(std::fs::read(&xd_main).unwrap(), xd_main_original_bytes);
        assert!(!modules::patch_dir(&xd).exists());
        assert!(!modules::patch_dir(&common).exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
