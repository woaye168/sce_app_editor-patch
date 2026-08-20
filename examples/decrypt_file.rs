//! 临时研究工具：单文件 TNND 解密（流式，输出到指定路径）。
//! 用法：cargo run --example decrypt_file -- <输入> <输出>

use std::fs;
use std::io::Write;

const MAGIC: [u8; 4] = *b"TNND";
const KEY: [u8; 10] = *b"CREATEEASY";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: decrypt_file <输入> <输出>");
        std::process::exit(1);
    }
    let mut raw = fs::read(&args[1]).unwrap();
    if raw.starts_with(&MAGIC) {
        raw.drain(..4);
        for (i, b) in raw.iter_mut().enumerate() {
            *b ^= KEY[i % KEY.len()];
        }
    }
    let mut out = fs::File::create(&args[2]).unwrap();
    out.write_all(&raw).unwrap();
    println!("完成：{} 字节 -> {}", raw.len(), args[2]);
}
