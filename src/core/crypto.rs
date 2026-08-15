//! 星火编辑器脚本包（common 包）的 XOR 加解密。
//!
//! 编辑器 `Res/_m/script/<版本>/script/` 下的 `.lua` 文件全部加密：
//! 前 4 字节为 magic 标识 `TNND`，其余字节与密钥 `CREATEEASY` 循环异或。

use std::fs;
use std::path::Path;

/// 加密文件头标识
pub const MAGIC: [u8; 4] = *b"TNND";
/// XOR 密钥
pub const KEY: [u8; 10] = *b"CREATEEASY";

/// 判断是否为加密格式（magic 头匹配）
pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(&MAGIC)
}

/// 解密：去掉 4 字节头，其余与密钥循环异或
pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, String> {
    if !is_encrypted(data) {
        return Err("不是预期的加密格式（缺少 TNND 头）".to_string());
    }
    Ok(xor(&data[MAGIC.len()..]))
}

/// 加密：明文异或后加上 4 字节头
pub fn encrypt(plain: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(plain.len() + MAGIC.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&xor(plain));
    out
}

fn xor(data: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect()
}

/// 读取并解密一个 lua 文件为文本
pub fn read_lua(path: &Path) -> Result<String, String> {
    let raw = fs::read(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let plain = decrypt(&raw)?;
    String::from_utf8(plain).map_err(|e| format!("{} 解密后不是有效 UTF-8: {e}", path.display()))
}

/// 加密写入 lua 文件（先写临时文件再替换，避免写一半损坏编辑器源文件）
pub fn write_lua(path: &Path, text: &str) -> Result<(), String> {
    write_atomic(path, &encrypt(text.as_bytes()))
}

/// 原子写：先写同目录临时文件，再替换目标
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("ep-tmp");
    fs::write(&tmp, data).map_err(|e| format!("写入临时文件 {} 失败: {e}", tmp.display()))?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("删除旧文件 {} 失败: {e}", path.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("替换 {} 失败: {e}", path.display()))?;
    Ok(())
}
