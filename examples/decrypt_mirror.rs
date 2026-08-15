//! 开发工具：把编辑器包目录整库解密到镜像目录（供源码研究/插槽文件制作使用）。
//!
//! 用法：
//!   cargo run --example decrypt_mirror -- <包目录> <镜像输出目录>
//!
//! 仅解密带 TNND 头的 .lua（明文原样复制），其他文件跳过。
//! 加密逻辑与 src/core/crypto.rs 一致（二进制 crate 无法被 example 引用，此处内联）。

use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"TNND";
const KEY: [u8; 10] = *b"CREATEEASY";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: decrypt_mirror <包目录> <镜像输出目录>");
        std::process::exit(1);
    }
    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);
    let mut decrypted = 0usize;
    let mut plain = 0usize;
    mirror(&src, &src, &dst, &mut decrypted, &mut plain);
    println!("完成：解密 {decrypted} 个，明文复制 {plain} 个 → {}", dst.display());
}

fn mirror(root: &Path, dir: &Path, dst: &Path, decrypted: &mut usize, plain: &mut usize) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            mirror(root, &path, dst, decrypted, plain);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("lua") {
            continue;
        }
        let raw = fs::read(&path).unwrap();
        let out = if raw.starts_with(&MAGIC) {
            *decrypted += 1;
            raw[4..]
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ KEY[i % KEY.len()])
                .collect::<Vec<u8>>()
        } else {
            *plain += 1;
            raw
        };
        let target = dst.join(path.strip_prefix(root).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, out).unwrap();
    }
}
