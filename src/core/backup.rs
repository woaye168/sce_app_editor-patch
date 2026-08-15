//! 备份机制：整库备份。应用补丁会改写库内大量文件（整库解密），
//! 因此在首次动手前把**整个包目录**原样备份；还原时整树覆盖回去。
//!
//! 备份目录放在**编辑器根目录**下（随编辑器数据走，应用卸载/重装不丢备份）：
//!
//! ```text
//! <编辑器根>/bgd_editor_patch/backup/
//!   api13/
//!     script_199/              # script 包（Res/_m/script/199/script）完整原始树
//!       common/isolation.lua
//!       ...
//!     script_199.manifest.json # 备份时间、来源路径
//!     xdeditor_160/            # xdeditor 包完整原始树
//!     xdeditor_160.manifest.json
//! ```
//!
//! 同一分组只备份首次，保证还原到真正的原始状态。

use super::ops;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// 备份根目录：<编辑器根>/bgd_editor_patch/backup
/// （可用环境变量 EDITOR_PATCH_BACKUP_DIR 覆盖，供测试使用）
pub fn backup_root(editor_root: &Path) -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("EDITOR_PATCH_BACKUP_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    Ok(editor_root.join("bgd_editor_patch").join("backup"))
}

fn lib_backup_dir(editor_root: &Path, group: &str) -> Result<PathBuf, String> {
    Ok(backup_root(editor_root)?.join(group))
}

/// 该库分组是否已有备份
pub fn has_backup(editor_root: &Path, group: &str) -> bool {
    lib_backup_dir(editor_root, group)
        .map(|p| p.is_dir())
        .unwrap_or(false)
}

/// 整库备份（已存在则跳过，返回是否为新备份）。`done` 每复制一个文件 +1。
/// - `group`：分组，如 `api13/script_199`
/// - `src_dir`：包目录，如 `Res/_m/script/199/script`
pub fn backup_lib(
    editor_root: &Path,
    group: &str,
    src_dir: &Path,
    done: &Arc<AtomicUsize>,
) -> Result<bool, String> {
    let dest = lib_backup_dir(editor_root, group)?;
    if dest.exists() {
        return Ok(false);
    }
    ops::copy_dir_recursive(src_dir, &dest, done)?;

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = format!(
        "{{\n  \"group\": \"{group}\",\n  \"source\": \"{}\",\n  \"backup_time\": {secs}\n}}\n",
        src_dir.display().to_string().replace('\\', "\\\\")
    );
    let _ = fs::write(format!("{}.manifest.json", dest.display()), manifest);
    Ok(true)
}

/// 整库还原：备份树覆盖回包目录（调用方负责删除补丁新增的目录）。
/// `done` 每复制一个文件 +1。
pub fn restore_lib(
    editor_root: &Path,
    group: &str,
    dst_dir: &Path,
    done: &Arc<AtomicUsize>,
) -> Result<(), String> {
    let backup = lib_backup_dir(editor_root, group)?;
    if !backup.is_dir() {
        return Err(format!("没有可用备份: {}", backup.display()));
    }
    ops::copy_dir_recursive(&backup, dst_dir, done)
}
