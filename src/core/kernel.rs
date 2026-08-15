//! 内核补丁：按库处理。
//!
//! 对每个目标库（`LIBS` 登记表）执行：
//! 1. **整库备份**（仅首次，见 backup 模块）
//! 2. **整库解密**：把库内加密的 .lua 原地替换为明文源码（明文文件跳过）。
//!    编辑器可以直接运行裸露源码，解密后便于查看/调试/打补丁。多线程并行 + 进度上报。
//! 3. **库专属文本补丁**：script 库额外解锁 common/isolation.lua 的 `= nil` 禁用行
//! 4. **入口插槽**：在库入口文件末尾（顶层 return 之前）注入
//!    `pcall(require, 'sce_app_editor-patch.main')`，并在库 require 根下创建补丁目录
//! 5. 首次创建补丁目录时启用默认勾选的模块
//!
//! 「还原补丁」：有备份的库整树覆盖还原 + 删除补丁目录。
//! 「状态检查」：入口插槽（+script 库解锁标记）是否在，编辑器升级覆盖后显示「未应用」。

use super::locate::EditorTarget;
use super::{backup, crypto, locate, log, modules, ops};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 注入块开始/结束标记（所有库共用）
pub const INJECT_BEGIN: &str = "-->> sce_app_editor-patch >>";
pub const INJECT_END: &str = "--<< sce_app_editor-patch <<";
/// 解锁行标记前缀
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 一个目标库（补丁打在该库入口点）
pub struct LibSpec {
    /// 包名（api_pak_version.json 中的键）
    pub pkg: &'static str,
    /// 界面显示名
    pub name: &'static str,
    /// 库内 require 根（相对包目录）：package.path 指向的目录
    pub require_root: &'static str,
    /// 入口文件（相对包目录）
    pub entry: &'static str,
}

/// 全部目标库（新增库补丁在此登记）
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
    /// 已应用（入口插槽在位，script 库还要求解锁标记在位）
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
    pub path: String,
}

/// 检查全部目标库状态
pub fn check(target: &EditorTarget) -> Vec<LibStatusInfo> {
    LIBS.iter()
        .map(|lib| {
            let version = target
                .package_version(lib.pkg)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "?".to_string());
            let has_backup = lib
                .backup_group(target)
                .map(|g| backup::has_backup(&target.editor_root, &g))
                .unwrap_or(false);
            let (path, status) = match lib.entry_path(target) {
                Ok(entry) => {
                    // read_lua 自适应加密/明文：未打补丁的加密入口也能正确读出内容
                    let s = match crypto::read_lua(&entry) {
                        Ok(text) if text.contains(INJECT_BEGIN) => {
                            // script 库额外要求 isolation 解锁标记在位
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
                version,
                status,
                has_backup,
                path,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- 进度

/// 后台任务进度（应用/还原共用）
pub struct TaskProgress {
    /// 当前阶段描述
    pub phase: String,
    /// 总文件数（备份+解密/还原复制）
    pub total: usize,
    /// 已处理文件数（原子计数，供并行任务上报）
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
    log::log(
        Some(project_root),
        Some(&target.editor_root),
        "INFO",
        "开始应用补丁",
    );

    // 预算：收集每个库的文件清单，计算总进度
    let mut jobs = Vec::new();
    let mut pre_errors: Vec<String> = Vec::new();
    for lib in LIBS {
        match (|| {
            let dir = lib.package_dir(&target)?;
            let group = lib.backup_group(&target)?;
            let needs_backup = !backup::has_backup(&target.editor_root, &group);
            let backup_count = if needs_backup {
                ops::collect_all_files(&dir).len()
            } else {
                0
            };
            let lua_files = ops::collect_lua_files(&dir);
            Ok::<_, String>(Job {
                lib,
                dir,
                group,
                needs_backup,
                backup_count,
                lua_files,
            })
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
                log::log(
                    Some(project_root),
                    Some(&target.editor_root),
                    "INFO",
                    &format!("库[{}]应用成功: {msg}", job.lib.pkg),
                );
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", job.lib.name));
                log::log(
                    Some(project_root),
                    Some(&target.editor_root),
                    "ERROR",
                    &format!("库[{}]应用失败: {e}", job.lib.pkg),
                );
            }
        }
    }
    if ok_count == 0 {
        return Err(format!("全部库应用失败：\n{}", lines.join("\n")));
    }
    Ok(lines.join("\n"))
}

/// 处理单个库：备份 → 整库解密 → 专属文本补丁 → 入口插槽 → 补丁目录/默认模块
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

    // 3. 库专属文本补丁：script 库解锁 isolation.lua
    if lib.pkg == "script" {
        let iso = job.dir.join("common").join("isolation.lua");
        let text = std::fs::read_to_string(&iso)
            .map_err(|e| format!("读取 {} 失败: {e}", iso.display()))?;
        let (text, unlocked) = transform_unlock(&text);
        crypto::write_atomic(&iso, text.as_bytes())?;
        parts.push(format!("解锁 {unlocked} 处禁用"));
    }

    // 4. 入口插槽（顶层 return 之前）
    let entry = lib.entry_path(target)?;
    let text = std::fs::read_to_string(&entry)
        .map_err(|e| format!("读取入口 {} 失败: {e}", entry.display()))?;
    let text = insert_slot(&text, &slot_block());
    crypto::write_atomic(&entry, text.as_bytes())?;
    parts.push("入口插槽已注入".to_string());

    // 5. 补丁目录：首次创建启用默认模块，否则按现状重建入口
    let root = lib.require_root_dir(target)?;
    if modules::patch_dir(&root).exists() {
        modules::regenerate_entry(&root)?;
        parts.push("保留已启用模块".to_string());
    } else {
        let defaults = modules::apply_defaults(&root)?;
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
    log::log(
        Some(project_root),
        Some(&target.editor_root),
        "INFO",
        "开始还原补丁",
    );

    // 预算：统计需要还原的文件总数
    struct Job<'a> {
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
        jobs.push(Job {
            lib,
            dir,
            group,
            file_count: count,
        });
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
                // 删除补丁目录（备份树里没有它，但覆盖复制不会删除多余文件）
                if let Ok(root) = job.lib.require_root_dir(&target) {
                    let patch_dir = modules::patch_dir(&root);
                    if patch_dir.exists() {
                        std::fs::remove_dir_all(&patch_dir)
                            .map_err(|e| format!("删除 {} 失败: {e}", patch_dir.display()))?;
                    }
                }
                lines.push(format!("✔ {}：已用备份整库还原", job.lib.name));
                log::log(
                    Some(project_root),
                    Some(&target.editor_root),
                    "INFO",
                    &format!("库[{}]还原成功", job.lib.pkg),
                );
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", job.lib.name));
                log::log(
                    Some(project_root),
                    Some(&target.editor_root),
                    "ERROR",
                    &format!("库[{}]还原失败: {e}", job.lib.pkg),
                );
            }
        }
    }
    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------- 文本转换

/// 入口插槽内容
fn slot_block() -> String {
    format!(
        "{INJECT_BEGIN}\n\
         -- 编辑器补丁插槽（由 sce_app_editor-patch 应用注入，请勿手改）\n\
         local __ep_ok, __ep_err = pcall(require, 'sce_app_editor-patch.main')\n\
         if not __ep_ok and log_file and log_file.info then\n\
         \x20   log_file.info('[sce_app_editor-patch] 框架入口加载失败: ' .. tostring(__ep_err))\n\
         end\n\
         {INJECT_END}"
    )
}

/// 把插槽注入文件末尾：若文件以顶层 return 语句结尾（单行或多行），插在 return 之前
fn insert_slot(text: &str, slot: &str) -> String {
    let base = remove_inject(text);
    let trimmed = base.trim_end().to_string();
    let lines: Vec<&str> = trimmed.lines().collect();
    match find_trailing_return(&lines) {
        Some(i) => {
            let mut out = lines[..i].join("\n").trim_end().to_string();
            out.push_str("\n\n");
            out.push_str(slot);
            out.push('\n');
            out.push_str(&lines[i..].join("\n"));
            out.push('\n');
            out
        }
        None => format!("{trimmed}\n\n{slot}\n"),
    }
}

/// 找文件末尾的顶层 return 语句起始行（Lua 要求 return 是块内最后一条语句，
/// 所以顶层 return 之后的行必然属于该 return 的延续，如花括号表）。
/// 用括号平衡验证完整性，验证不过则返回 None（退化为文件末尾追加）。
fn find_trailing_return(lines: &[&str]) -> Option<usize> {
    let last = lines.iter().rposition(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("--")
    })?;
    for (i, line) in lines.iter().enumerate().take(last + 1).rev() {
        let t = line.trim_end();
        if t.starts_with("return")
            && (t.len() == 6 || t.as_bytes()[6].is_ascii_whitespace())
            && !line.starts_with(char::is_whitespace)
        {
            // 验证 i..=last 括号平衡（多行 return 的表/参数完整闭合）
            let mut depth: i32 = 0;
            for l in lines.iter().take(last + 1).skip(i) {
                for ch in l.chars() {
                    match ch {
                        '{' | '(' | '[' => depth += 1,
                        '}' | ')' | ']' => depth -= 1,
                        _ => {}
                    }
                }
            }
            return if depth == 0 { Some(i) } else { None };
        }
    }
    None
}

/// 移除已有注入块（含标记行之间的所有内容）
fn remove_inject(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == INJECT_BEGIN {
            inside = true;
            continue;
        }
        if trimmed == INJECT_END {
            inside = false;
            continue;
        }
        if !inside {
            out.push(line);
        }
    }
    out.join("\n")
}

/// 解锁转换：注释掉所有 `xxx = nil` 禁用行，返回 (新文本, 解锁数量)
fn transform_unlock(text: &str) -> (String, usize) {
    let mut count = 0;
    let out = text
        .lines()
        .map(|line| {
            if is_nil_disable_line(line) {
                count += 1;
                // 保留原缩进：标记前缀插在原行前
                format!("{UNLOCK_MARK}{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (out, count)
}

/// 判断是否为「标识符路径 = nil」禁用行（如 `io.popen = nil`、`_G.package.loadlib = nil`）
/// 已注释行、`local x = nil` 等不算
fn is_nil_disable_line(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with("--") {
        return false;
    }
    let Some(lhs) = t.strip_suffix("nil") else {
        return false;
    };
    let Some(lhs) = lhs.trim_end().strip_suffix('=') else {
        return false;
    };
    let lhs = lhs.trim();
    if lhs.is_empty() {
        return false;
    }
    let first = lhs.chars().next().unwrap();
    (first.is_ascii_alphabetic() || first == '_')
        && lhs
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

// 私有 Job 类型仅给 apply_all/apply_lib 用（定义在函数内会借用问题，提升到这里）
struct Job<'a> {
    lib: &'a LibSpec,
    dir: PathBuf,
    group: String,
    needs_backup: bool,
    backup_count: usize,
    lua_files: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_nil_disable_line() {
        assert!(is_nil_disable_line("    io.popen = nil"));
        assert!(is_nil_disable_line("os.execute = nil"));
        assert!(is_nil_disable_line("    _G.package.loadlib = nil"));
        assert!(is_nil_disable_line("cmsg_pack.set_max_pack_byte_count = nil"));
        assert!(!is_nil_disable_line("-- io.unzip_file = nil"));
        assert!(!is_nil_disable_line("local x = nil"));
        assert!(!is_nil_disable_line("local write = io.write"));
        assert!(!is_nil_disable_line("io.create_dir('.')"));
        assert!(!is_nil_disable_line(""));
    }

    #[test]
    fn test_transform_unlock() {
        let src = "local a = 1\n    io.popen = nil\n    os.execute = nil\n-- io.unzip_file = nil\n";
        let (out, count) = transform_unlock(src);
        assert_eq!(count, 2);
        assert!(out.contains("-- [sce_app_editor-patch 解锁]     io.popen = nil"));
        let (_out2, count2) = transform_unlock(&out);
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_remove_inject() {
        let src = format!("line1\n{}\nabc\ndef\n{}\nline2\n", INJECT_BEGIN, INJECT_END);
        assert_eq!(remove_inject(&src), "line1\nline2");
    }

    #[test]
    fn test_insert_slot_no_return() {
        // common/init.lua 形态：无 return，末尾追加
        let src = "if not _G.log_file then\n    _G.log_file = log\nend\n\nrequire 'main'\n";
        let out = insert_slot(src, "SLOT");
        assert!(out.ends_with("require 'main'\n\nSLOT\n"));
    }

    #[test]
    fn test_insert_slot_single_line_return() {
        // menu_bar.lua 形态：return xxx 结尾，插槽在 return 之前
        let src = "window_title_bar.register('帮助/文档', f)\n\nreturn window_title_bar\n";
        let out = insert_slot(src, "SLOT");
        assert_eq!(
            out,
            "window_title_bar.register('帮助/文档', f)\n\nSLOT\nreturn window_title_bar\n"
        );
    }

    #[test]
    fn test_insert_slot_multi_line_return() {
        // xdeditor/main.lua 形态：return { ... } 多行结尾
        let src = "local a = 1\n    return\n\nreturn {\n    x = 1,\n    y = 2,\n}\n";
        let out = insert_slot(src, "SLOT");
        assert_eq!(out, "local a = 1\n    return\n\nSLOT\nreturn {\n    x = 1,\n    y = 2,\n}\n");
    }

    #[test]
    fn test_insert_slot_idempotent() {
        let src = "require 'main'\n";
        let slot = format!("{INJECT_BEGIN}\nbody\n{INJECT_END}");
        let once = insert_slot(src, &slot);
        let twice = insert_slot(&once, &slot);
        assert_eq!(once, twice);
    }

    #[test]
    fn test_crypto_round_trip() {
        let plain = "hello 星火编辑器";
        let enc = crypto::encrypt(plain.as_bytes());
        assert!(crypto::is_encrypted(&enc));
        assert_eq!(crypto::decrypt(&enc).unwrap(), plain.as_bytes());
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

        // script 包：加密 isolation.lua + 加密 init.lua + 一个明文文件
        let common = editor_root.join("Res/_m/script/199/script/common");
        std::fs::create_dir_all(&common).unwrap();
        let iso_original = "local util = require 'base.util'\nif __lua_state_name == 'StateGame' then\n    io.popen = nil\nend\n";
        let iso = common.join("isolation.lua");
        std::fs::write(&iso, crypto::encrypt(iso_original.as_bytes())).unwrap();
        let iso_original_bytes = std::fs::read(&iso).unwrap();
        let init = common.join("init.lua");
        std::fs::write(&init, crypto::encrypt(b"require 'main'\n")).unwrap();
        let plain_file = common.join("plain_note.lua");
        std::fs::write(&plain_file, "-- 本来就是明文\n").unwrap();

        // xdeditor 包：明文 main.lua（多行 return 结尾）
        let xd = editor_root.join("Res/_m/xdeditor/160/xdeditor");
        std::fs::create_dir_all(&xd).unwrap();
        let menu_dir = xd.join("ui");
        std::fs::create_dir_all(&menu_dir).unwrap();
        std::fs::write(menu_dir.join("menu_bar.lua"), "local M = {}\nreturn M\n").unwrap();
        let xd_main_original = "require '@common.base'\n\nreturn {\n    a = 1,\n}\n";
        let xd_main = xd.join("main.lua");
        std::fs::write(&xd_main, xd_main_original).unwrap();

        // 应用（同步跑 worker 逻辑）
        let progress = new_progress();
        let msg = apply_all(&project, &progress).unwrap();
        assert!(msg.contains("解锁 1 处"), "{msg}");
        assert!(msg.contains("默认启用模块: menu_bgd"), "{msg}");

        // 状态：两库均已应用
        let target = locate::locate(&project).unwrap();
        let statuses = check(&target);
        assert!(statuses.iter().all(|s| s.status == LibStatus::Applied), "全部已应用: {msg}");

        // 整库已解密为明文
        assert!(!crypto::is_encrypted(&std::fs::read(&iso).unwrap()));
        assert!(!crypto::is_encrypted(&std::fs::read(&init).unwrap()));
        // 明文文件未被破坏
        assert_eq!(std::fs::read_to_string(&plain_file).unwrap(), "-- 本来就是明文\n");

        // script 入口插槽 + 解锁
        let init_text = std::fs::read_to_string(&init).unwrap();
        assert!(init_text.contains(INJECT_BEGIN));
        let iso_text = std::fs::read_to_string(&iso).unwrap();
        assert!(iso_text.contains(UNLOCK_MARK));

        // xdeditor 入口插槽在 return 之前
        let xd_text = std::fs::read_to_string(&xd_main).unwrap();
        let slot_pos = xd_text.find(INJECT_BEGIN).unwrap();
        let ret_pos = xd_text.find("return {").unwrap();
        assert!(slot_pos < ret_pos, "插槽必须在 return 之前:\n{xd_text}");

        // 默认模块已启用（明文写入 xdeditor 补丁目录）
        let menu_module = modules::patch_dir(&xd).join("menu_bgd").join("main.lua");
        assert!(menu_module.is_file());
        assert!(!crypto::is_encrypted(&std::fs::read(&menu_module).unwrap()));

        // 再应用：幂等（保留模块、解锁不重复）
        let progress2 = new_progress();
        let msg2 = apply_all(&project, &progress2).unwrap();
        assert!(msg2.contains("解锁 0 处"), "{msg2}");
        assert!(msg2.contains("保留已启用模块"), "{msg2}");

        // 模拟编辑器升级覆盖：入口换成全新加密原始文件 → 检测为未应用
        std::fs::write(&init, crypto::encrypt(b"require 'main'\n")).unwrap();
        let target = locate::locate(&project).unwrap();
        let statuses = check(&target);
        let script_status = statuses.iter().find(|s| s.pkg == "script").unwrap();
        assert_eq!(script_status.status, LibStatus::NotApplied, "覆盖后应检测为未应用");

        // 还原：整库字节级还原
        let progress3 = new_progress();
        restore_all(&project, &progress3).unwrap();
        assert_eq!(std::fs::read(&iso).unwrap(), iso_original_bytes);
        assert!(crypto::is_encrypted(&std::fs::read(&init).unwrap()));
        assert_eq!(std::fs::read_to_string(&xd_main).unwrap(), xd_main_original);
        assert!(!modules::patch_dir(&xd).exists());
        assert!(!modules::patch_dir(&common).exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
