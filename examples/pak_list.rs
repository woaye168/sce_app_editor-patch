//! 临时研究工具：解析 Urho3D UPAK 索引，输出条目清单（名字/偏移/大小）。
//! 用法：cargo run --example pak_list -- <已解密的pak> <输出txt>

use std::fs;
use std::io::Write;

fn u32le(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: pak_list <pak> <输出txt>");
        std::process::exit(1);
    }
    let data = fs::read(&args[1]).unwrap();
    assert!(&data[0..4] == b"UPAK", "不是 UPAK 文件");
    let count = u32le(&data, 4) as usize;
    let mut p = 12; // magic + count + checksum
    let mut out = fs::File::create(&args[2]).unwrap();
    writeln!(out, "条目数: {count}").unwrap();
    for _ in 0..count {
        let end = data[p..].iter().position(|&b| b == 0).unwrap() + p;
        let name = String::from_utf8_lossy(&data[p..end]).into_owned();
        p = end + 1;
        let offset = u32le(&data, p);
        let size = u32le(&data, p + 4);
        p += 8;
        writeln!(out, "{offset:>10} {size:>10} {name}").unwrap();
    }
    println!("完成：{count} 条目 -> {}", args[2]);
}
