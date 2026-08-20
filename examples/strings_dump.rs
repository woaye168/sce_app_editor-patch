//! 临时研究工具：dump 二进制文件中的可打印 ASCII 字符串（每行一条，长度>=4）。
//! 用法：cargo run --example strings_dump -- <二进制文件> <输出txt>

use std::fs;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: strings_dump <二进制文件> <输出txt>");
        std::process::exit(1);
    }
    let data = fs::read(&args[1]).unwrap();
    let mut out = fs::File::create(&args[2]).unwrap();
    let mut cur = String::new();
    for &b in &data {
        if (0x20..0x7f).contains(&b) {
            cur.push(b as char);
        } else {
            if cur.len() >= 4 {
                writeln!(out, "{cur}").unwrap();
            }
            cur.clear();
        }
    }
    println!("完成");
}
