//! 内核补丁：解除 isolation.lua 的函数禁用，并注入补丁框架入口。
//!
//! 「应用补丁」流程（幂等，可重复执行）：
//! 1. 解密 isolation.lua（格式不符则中止，绝不蛮干）
//! 2. 备份原始加密字节（仅首次，见 backup 模块）
//! 3. 解锁：注释掉所有 `xxx = nil` 禁用行（保留原文，带标记前缀）
//! 4. 末尾注入 `sce_app_editor-patch.main` 入口（标记块包裹，重复应用先除旧）
//! 5. 加密写回（原子替换），并重建补丁框架入口文件
//!
//! 「还原补丁」流程：
//! 1. 用备份原样还原 isolation.lua
//! 2. 删除 common 下的 sce_app_editor-patch 目录（含所有已启用模块）

use super::{backup, crypto, modules};
use super::locate::EditorTarget;
use std::fs;

/// 注入块开始/结束标记
pub const INJECT_BEGIN: &str = "-->> sce_app_editor-patch >>";
pub const INJECT_END: &str = "--<< sce_app_editor-patch <<";
/// 解锁行标记前缀
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 内核状态
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum KernelStatus {
    /// 未应用（原始状态）
    NotApplied,
    /// 已应用（含注入标记）
    Applied,
    /// 无法识别（文件缺失或非加密格式）
    Unknown,
}

/// 查询内核状态
pub fn status(target: &EditorTarget) -> KernelStatus {
    let iso = target.isolation_lua();
    let Ok(raw) = fs::read(&iso) else {
        return KernelStatus::Unknown;
    };
    if !crypto::is_encrypted(&raw) {
        return KernelStatus::Unknown;
    }
    match crypto::read_lua(&iso) {
        Ok(text) if text.contains(INJECT_BEGIN) => KernelStatus::Applied,
        Ok(_) => KernelStatus::NotApplied,
        Err(_) => KernelStatus::Unknown,
    }
}

/// 应用内核补丁，返回结果描述
pub fn apply(target: &EditorTarget) -> Result<String, String> {
    let iso = target.isolation_lua();
    let raw = fs::read(&iso).map_err(|e| format!("读取 {} 失败: {e}", iso.display()))?;
    if !crypto::is_encrypted(&raw) {
        return Err("isolation.lua 不是预期的加密格式，为安全起见已中止（编辑器源文件未被修改）"
            .to_string());
    }

    // 先备份再动手（仅首次真正写入）
    let new_backup = backup::backup_file(&target.backup_tag(), "isolation.lua", &iso)?;

    let text = crypto::read_lua(&iso)?;
    // 幂等：先移除旧注入块
    let text = remove_inject(&text);
    // 解锁 = nil 禁用
    let (text, unlocked) = transform_unlock(&text);
    // 追加注入块
    let text = format!("{}\n\n{}\n", text.trim_end(), inject_block());
    crypto::write_lua(&iso, &text)?;

    // 重建补丁框架入口（保留已启用模块）
    modules::regenerate_entry(&target.common_dir)?;

    Ok(if new_backup {
        format!("已备份原文件，解锁 {unlocked} 处禁用，注入框架入口")
    } else {
        format!("解锁 {unlocked} 处禁用，注入框架入口（沿用已有备份）")
    })
}

/// 还原内核补丁，返回结果描述
pub fn restore(target: &EditorTarget) -> Result<String, String> {
    let iso = target.isolation_lua();
    backup::restore_file(&target.backup_tag(), "isolation.lua", &iso)?;

    // 删除补丁框架目录（含所有已启用模块）
    let patch_dir = modules::patch_dir(&target.common_dir);
    if patch_dir.exists() {
        fs::remove_dir_all(&patch_dir)
            .map_err(|e| format!("删除 {} 失败: {e}", patch_dir.display()))?;
    }
    Ok("已用备份还原 isolation.lua，并移除补丁框架目录".to_string())
}

/// 注入块内容
fn inject_block() -> String {
    format!(
        "{INJECT_BEGIN}\n\
         -- 编辑器补丁框架入口（由 sce_app_editor-patch 应用注入，请勿手改）\n\
         local __ep_ok, __ep_err = pcall(require, 'sce_app_editor-patch.main')\n\
         if not __ep_ok then\n\
         \x20   log_file.info('[sce_app_editor-patch] 框架入口加载失败: ' .. tostring(__ep_err))\n\
         end\n\
         {INJECT_END}"
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{crypto, locate::EditorTarget};

    #[test]
    fn test_is_nil_disable_line() {
        assert!(is_nil_disable_line("    io.popen = nil"));
        assert!(is_nil_disable_line("os.execute = nil"));
        assert!(is_nil_disable_line("    _G.package.loadlib = nil"));
        assert!(is_nil_disable_line("cmsg_pack.set_max_pack_byte_count = nil"));
        // 已注释 / local 赋值 / 其他语句不算
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
        assert!(out.contains("-- [sce_app_editor-patch 解锁]     os.execute = nil"));
        // 已注释行不重复处理
        assert!(out.contains("-- io.unzip_file = nil"));
        // 重复执行幂等
        let (_out2, count2) = transform_unlock(&out);
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_remove_inject() {
        let src = format!("line1\n{}\nabc\ndef\n{}\nline2\n", INJECT_BEGIN, INJECT_END);
        let out = remove_inject(&src);
        assert_eq!(out, "line1\nline2");
    }

    #[test]
    fn test_crypto_round_trip() {
        let plain = "hello 星火编辑器";
        let enc = crypto::encrypt(plain.as_bytes());
        assert!(crypto::is_encrypted(&enc));
        assert!(enc.starts_with(b"TNND"));
        let dec = crypto::decrypt(&enc).unwrap();
        assert_eq!(dec, plain.as_bytes());
    }

    /// 端到端：临时目录上 应用→状态→再应用（幂等）→还原 全流程
    #[test]
    fn test_apply_restore_flow() {
        let base = std::env::temp_dir().join(format!("editor_patch_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let common = base.join("common");
        std::fs::create_dir_all(&common).unwrap();
        let backup_dir = base.join("backup");
        std::env::set_var("EDITOR_PATCH_BACKUP_DIR", &backup_dir);

        // 造一个加密的 isolation.lua
        let original = "local util = require 'base.util'\nif __lua_state_name == 'StateGame' then\n    io.popen = nil\n    os.execute = nil\nend\n";
        let iso = common.join("isolation.lua");
        std::fs::write(&iso, crypto::encrypt(original.as_bytes())).unwrap();
        let original_bytes = std::fs::read(&iso).unwrap();

        let target = EditorTarget {
            api_version: "13".to_string(),
            editor_root: base.clone(),
            script_version: 199,
            common_dir: common.clone(),
        };

        // 应用
        let msg = apply(&target).unwrap();
        assert!(msg.contains("解锁 2 处"), "{msg}");
        assert_eq!(status(&target), KernelStatus::Applied);
        let patched = crypto::read_lua(&iso).unwrap();
        assert!(patched.contains(INJECT_BEGIN));
        assert!(patched.contains("pcall(require, 'sce_app_editor-patch.main')"));
        assert!(patched.contains("-- [sce_app_editor-patch 解锁]     io.popen = nil"));
        // 备份已创建
        assert!(crate::core::backup::has_backup("api13_script199", "isolation.lua"));
        // 框架入口已生成（加密）
        let entry = common.join("sce_app_editor-patch").join("main.lua");
        assert!(crypto::is_encrypted(&std::fs::read(&entry).unwrap()));

        // 再应用：幂等，解锁数不重复
        let msg2 = apply(&target).unwrap();
        assert!(msg2.contains("解锁 0 处"), "{msg2}");
        assert_eq!(status(&target), KernelStatus::Applied);

        // 还原：字节级一致
        restore(&target).unwrap();
        assert_eq!(std::fs::read(&iso).unwrap(), original_bytes);
        assert_eq!(status(&target), KernelStatus::NotApplied);
        assert!(!common.join("sce_app_editor-patch").exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
