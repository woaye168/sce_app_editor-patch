//! 临时研究工具：就地解密目录树中所有带 TNND 头的文件（不限扩展名）。
//! 用法：cargo run --example decrypt_inplace -- <目录>
//! 仅用于 .editor_src_mirror 研究副本，严禁指向编辑器原始目录。

use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"TNND";
const KEY: [u8; 10] = *b"CREATEEASY";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("用法: decrypt_inplace <目录>");
        std::process::exit(1);
    }
    let root = PathBuf::from(&args[1]);
    assert!(
        root.to_string_lossy().contains(".editor_src_mirror"),
        "安全护栏：只允许在 .editor_src_mirror 研究副本内就地解密"
    );
    let mut decrypted = 0usize;
    walk(&root, &mut decrypted);
    println!("完成：就地解密 {decrypted} 个文件");
}

fn walk(dir: &Path, decrypted: &mut usize) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, decrypted);
            continue;
        }
        let Ok(raw) = fs::read(&path) else { continue };
        if !raw.starts_with(&MAGIC) {
            continue;
        }
        let out: Vec<u8> = raw[4..]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ KEY[i % KEY.len()])
            .collect();
        fs::write(&path, out).unwrap();
        *decrypted += 1;
    }
}
