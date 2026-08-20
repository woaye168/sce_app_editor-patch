//! 临时研究工具：解包 SCE UPAK（条目 = 名字\0 + u32 offset + u32 size + u32 checksum）到目录。
//! 用法：cargo run --example pak_extract -- <已解密的pak> <输出目录>

use std::fs;
use std::path::Path;

fn u32le(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: pak_extract <pak> <输出目录>");
        std::process::exit(1);
    }
    let data = fs::read(&args[1]).unwrap();
    assert!(&data[0..4] == b"UPAK", "不是 UPAK 文件");
    let count = u32le(&data, 4) as usize;
    let out_root = Path::new(&args[2]);
    let mut p = 12usize;
    let mut ok = 0usize;
    for _ in 0..count {
        let end = data[p..].iter().position(|&b| b == 0).unwrap() + p;
        let name = String::from_utf8_lossy(&data[p..end]).into_owned();
        p = end + 1;
        let offset = u32le(&data, p) as usize;
        let size = u32le(&data, p + 4) as usize;
        p += 12; // offset + size + checksum
        // 防路径穿越
        let safe = name.replace("..", "__");
        let target = out_root.join(&safe);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, &data[offset..offset + size]).unwrap();
        ok += 1;
    }
    println!("完成：{ok}/{count} 条目 -> {}", out_root.display());
}
