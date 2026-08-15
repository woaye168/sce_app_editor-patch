//! 备份机制：但凡补丁要修改星火编辑器源文件，先把原文件原样备份。
//!
//! 备份目录放在**编辑器根目录**下（随编辑器数据走，应用卸载/重装不丢备份），
//! 按「编辑器版本 / 包_版本」分组，支持多库多文件：
//!
//! ```text
//! <编辑器根>/bgd_editor_patch/backup/
//!   api13/
//!     script_199/common/isolation.lua       # 原始字节，原样恢复即可
//!     script_199/common/isolation.lua.manifest.json
//!     xdeditor_160/ui/menu_bar.lua
//!     xdeditor_160/ui/menu_bar.lua.manifest.json
//! ```
//!
//! 同一文件只备份首次（之后即使重复应用补丁也不覆盖），保证还原到真正的原始文件。

use std::fs;
use std::path::{Path, PathBuf};
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

fn backup_path(editor_root: &Path, group: &str, rel: &str) -> Result<PathBuf, String> {
    Ok(backup_root(editor_root)?.join(group).join(rel))
}

/// 是否已有备份
pub fn has_backup(editor_root: &Path, group: &str, rel: &str) -> bool {
    backup_path(editor_root, group, rel)
        .map(|p| p.is_file())
        .unwrap_or(false)
}

/// 备份文件（已存在则跳过，返回是否为新备份）
/// - `group`：分组，如 `api13/script_199`
/// - `rel`：包内相对路径，如 `common/isolation.lua`
pub fn backup_file(editor_root: &Path, group: &str, rel: &str, src: &Path) -> Result<bool, String> {
    let dest = backup_path(editor_root, group, rel)?;
    if dest.exists() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建备份目录失败: {e}"))?;
    }
    fs::copy(src, &dest).map_err(|e| format!("备份 {} 失败: {e}", src.display()))?;

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = format!(
        "{{\n  \"file\": \"{rel}\",\n  \"source\": \"{}\",\n  \"backup_time\": {secs}\n}}\n",
        src.display().to_string().replace('\\', "\\\\")
    );
    let manifest_path = dest.with_extension("lua.manifest.json");
    let _ = fs::write(manifest_path, manifest);
    Ok(true)
}

/// 用备份还原目标文件（原子替换）
pub fn restore_file(editor_root: &Path, group: &str, rel: &str, dest: &Path) -> Result<(), String> {
    let backup = backup_path(editor_root, group, rel)?;
    if !backup.is_file() {
        return Err(format!("没有可用备份: {}", backup.display()));
    }
    let data = fs::read(&backup).map_err(|e| format!("读取备份 {} 失败: {e}", backup.display()))?;
    super::crypto::write_atomic(dest, &data)?;
    Ok(())
}
