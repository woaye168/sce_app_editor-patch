//! 备份机制：但凡补丁要修改星火编辑器源文件，先把原文件原样备份。
//!
//! 备份目录自持于应用安装目录下（exe 同级），按「编辑器版本 + 包版本」分组：
//!
//! ```text
//! <exe目录>/backup/
//!   api13_script199/
//!     isolation.lua      # 原始加密字节，原样恢复即可
//!     manifest.json      # 备份时间、来源路径
//! ```
//!
//! 同一分组只备份首次（之后即使重复应用补丁也不覆盖），保证还原到真正的原始文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 备份根目录：<exe目录>/backup（可用环境变量 EDITOR_PATCH_BACKUP_DIR 覆盖，供测试使用）
pub fn backup_root() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("EDITOR_PATCH_BACKUP_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let exe = std::env::current_exe().map_err(|e| format!("获取 exe 路径失败: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "exe 没有父目录".to_string())?;
    Ok(dir.join("backup"))
}

fn backup_path(tag: &str, name: &str) -> Result<PathBuf, String> {
    Ok(backup_root()?.join(tag).join(name))
}

/// 是否已有备份
pub fn has_backup(tag: &str, name: &str) -> bool {
    backup_path(tag, name).map(|p| p.is_file()).unwrap_or(false)
}

/// 备份文件（已存在则跳过，返回是否为新备份）
pub fn backup_file(tag: &str, name: &str, src: &Path) -> Result<bool, String> {
    let dest = backup_path(tag, name)?;
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
        "{{\n  \"file\": \"{name}\",\n  \"source\": \"{}\",\n  \"backup_time\": {secs}\n}}\n",
        src.display().to_string().replace('\\', "\\\\")
    );
    let manifest_path = dest.with_extension("manifest.json");
    let _ = fs::write(manifest_path, manifest);
    Ok(true)
}

/// 用备份还原目标文件（原子替换）
pub fn restore_file(tag: &str, name: &str, dest: &Path) -> Result<(), String> {
    let backup = backup_path(tag, name)?;
    if !backup.is_file() {
        return Err(format!("没有可用备份: {}", backup.display()));
    }
    let data = fs::read(&backup).map_err(|e| format!("读取备份 {} 失败: {e}", backup.display()))?;
    super::crypto::write_atomic(dest, &data)?;
    Ok(())
}
