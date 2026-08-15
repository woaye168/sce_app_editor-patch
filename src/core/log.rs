//! 日志：记录定位/应用/还原/模块启停等动作。
//!
//! 日志随备份一起放在编辑器根目录下（应用卸载不丢）：
//! `<编辑器根>/bgd_editor_patch/log/editor-patch.log`
//! 定位不到编辑器根目录时（如项目未选择），退回 exe 同级 `log/` 目录。

use std::fs;
use std::path::{Path, PathBuf};

/// 日志文件路径
pub fn log_path(editor_root: Option<&Path>) -> PathBuf {
    if let Some(root) = editor_root {
        return root
            .join("bgd_editor_patch")
            .join("log")
            .join("editor-patch.log");
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("log").join("editor-patch.log")
}

/// 追加一行日志（失败静默，不影响主流程）
pub fn log(editor_root: Option<&Path>, level: &str, message: &str) {
    let path = log_path(editor_root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[{timestamp}] [{level}] {message}\n");
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}
