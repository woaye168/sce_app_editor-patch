//! 插槽注入/解锁变换（0.5.3 F4：从 make_slots 工具下沉为内核共用实现，单一事实源）。
//!
//! 用途两端：
//! - `examples/make_slots`：离线生成 slots/<库>/<版本>/ 插槽文件 + slot.manifest.json
//! - 内核运行时（kernel.rs 三级回退第 3 级）：编辑器包升级无精确插槽时，
//!   对新版本解码文本现场执行同款注入变换
//!
//! 处理链：解密（TNND 头则解）→ 若 UTF-8 非法则按 GBK 转 UTF-8 → 注入/解锁变换。

use sha2::{Digest, Sha256};

/// 加密文件 magic 头
const MAGIC: [u8; 4] = *b"TNND";
/// XOR 密钥
const KEY: [u8; 10] = *b"CREATEEASY";

/// 插槽开始/结束标记（注入块包裹标记）
pub const INJECT_BEGIN: &str = "-->> sce_app_editor-patch >>";
pub const INJECT_END: &str = "--<< sce_app_editor-patch <<";
/// 解锁行标记前缀（isolation.lua 每个被解禁行的行首注释）
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 解码官方源文本：XOR 解密（按 magic 逐文件判断）→ UTF-8 非法则 GBK 转 UTF-8。
/// 明文输入原样返回（尽力 UTF-8，否则 GBK 转换）。
pub fn decode_source(raw: &[u8]) -> String {
    let bytes: Vec<u8> = if raw.starts_with(&MAGIC) {
        raw[4..]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ KEY[i % KEY.len()])
            .collect()
    } else {
        raw.to_vec()
    };
    match String::from_utf8(bytes.clone()) {
        Ok(t) => t,
        Err(_) => {
            let (t, _, _) = encoding_rs::GBK.decode(&bytes);
            t.into_owned()
        }
    }
}

/// 源文本内容哈希（sha256 hex）。复用判定：新版本解码源与 slots manifest 记录的源哈希比对。
pub fn source_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 入口插槽块内容（库入口顶层 return 之前注入）
pub fn slot_block() -> String {
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

/// 在源文本顶层 return 之前注入插槽块；找不到顶层 return 时追加到末尾。
pub fn insert_slot(text: &str, slot: &str) -> String {
    let trimmed = text.trim_end();
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

/// 找顶层 return 行号（深度配平校验，避免匹配到表/函数体内的 return）
fn find_trailing_return(lines: &[&str]) -> Option<usize> {
    let last = lines
        .iter()
        .rposition(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("--")
        })?;
    for (i, line) in lines.iter().enumerate().take(last + 1).rev() {
        let t = line.trim_end();
        if t.starts_with("return")
            && (t.len() == 6 || t.as_bytes()[6].is_ascii_whitespace())
            && !line.starts_with(char::is_whitespace)
        {
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

/// 移除文本中已存在的插槽块（INJECT_BEGIN..INJECT_END 含标记行，含其前的空行）。
/// 用于对「已打补丁」的源做幂等重生成（make_slots 对运行中的编辑器根重跑）。
pub fn strip_slot(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.contains(INJECT_BEGIN));
    let end = lines.iter().position(|l| l.contains(INJECT_END));
    let (Some(b), Some(e)) = (begin, end) else {
        return text.to_string();
    };
    if e < b {
        return text.to_string();
    }
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    out.extend_from_slice(&lines[..b]);
    out.extend_from_slice(&lines[e + 1..]);
    // 只处理拼接处的空行折叠（插入时块前固定补了一个空行）；其余位置的空行原样保留
    let mut collapsed: Vec<&str> = Vec::with_capacity(out.len());
    for (i, l) in out.iter().enumerate() {
        // i == b 即拼接点第一行：若它与上一行均为空行，跳过它（等效撤销插入前的补空行）
        if i == b
            && l.trim().is_empty()
            && b > 0
            && out[b - 1].trim().is_empty()
        {
            continue;
        }
        collapsed.push(l);
    }
    collapsed.join("\n")
}

/// 移除解锁行标记前缀（isolation 幂等重生成用）
pub fn strip_unlock(text: &str) -> String {
    text.lines()
        .map(|l| l.strip_prefix(UNLOCK_MARK).unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// isolation.lua 解锁变换：把 `xxx = nil` 禁用行行首加解锁标记注释。
/// 返回 (变换后文本, 解锁行数)。
pub fn transform_unlock(text: &str) -> (String, usize) {
    let mut count = 0;
    let out = text
        .lines()
        .map(|line| {
            if is_nil_disable_line(line) {
                count += 1;
                format!("{UNLOCK_MARK}{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (out, count)
}

/// 判断 `标识符[.字段...] = nil` 形态的禁用行（跳过注释行）
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

    #[test]
    fn test_insert_slot_before_trailing_return() {
        let src = "local M = {}\nfunction M.x() end\n\nreturn M\n";
        let out = insert_slot(src, "--SLOT--");
        let pos_slot = out.find("--SLOT--").unwrap();
        let pos_ret = out.rfind("return M").unwrap();
        assert!(pos_slot < pos_ret, "插槽必须在顶层 return 之前: {out}");
    }

    #[test]
    fn test_insert_slot_no_return_appends() {
        let src = "local x = 1\n";
        let out = insert_slot(src, "--SLOT--");
        assert!(out.contains("local x = 1\n\n--SLOT--\n"));
    }

    #[test]
    fn test_transform_unlock() {
        let src = "-- comment = nil\nos.exit = nil\nlocal x = nil\nio.open = nil\n";
        let (out, n) = transform_unlock(src);
        assert_eq!(n, 2, "只解禁顶层赋值 nil 行（注释行与 local 声明不动）: {out}");
        assert!(out.contains("-- [sce_app_editor-patch 解锁] os.exit = nil"));
        assert!(out.contains("-- [sce_app_editor-patch 解锁] io.open = nil"));
        assert!(out.contains("-- comment = nil"));
        assert!(out.lines().any(|l| l == "local x = nil"));
    }

    #[test]
    fn test_decode_plain_and_hash_stable() {
        let raw = b"hello\n";
        assert_eq!(decode_source(raw), "hello\n");
        assert_eq!(source_hash("abc"), source_hash("abc"));
        assert_ne!(source_hash("abc"), source_hash("abd"));
    }
}
