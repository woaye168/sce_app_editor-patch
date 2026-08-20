//! 临时研究工具：列出 PE 文件（exe/dll）的导出符号表。
//! 用法：cargo run --example pe_exports -- <pe文件>

use std::fs;

fn u16le(b: &[u8], p: usize) -> u16 { u16::from_le_bytes([b[p], b[p + 1]]) }
fn u32le(b: &[u8], p: usize) -> u32 { u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]) }

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = fs::read(&args[1]).unwrap();
    assert!(&data[0..2] == b"MZ", "不是 PE 文件");
    let pe_off = u32le(&data, 0x3c) as usize;
    assert!(&data[pe_off..pe_off + 4] == b"PE\0\0", "PE 头无效");
    let num_sections = u16le(&data, pe_off + 6) as usize;
    let opt_size = u16le(&data, pe_off + 20) as usize;
    let opt_off = pe_off + 24;
    let magic = u16le(&data, opt_off);
    let is64 = magic == 0x20b;
    // 导出表 data directory（EXPORT=index 0）：PE32+ offset 112，PE32 offset 96
    let exp_rva = u32le(&data, opt_off + if is64 { 112 } else { 96 }) as usize;
    if exp_rva == 0 {
        println!("无导出表");
        return;
    }
    // RVA → 文件偏移：遍历节表
    let sec_off = opt_off + opt_size;
    let rva2off = |rva: usize| -> usize {
        for i in 0..num_sections {
            let s = sec_off + i * 40;
            let vsize = u32le(&data, s + 8) as usize;
            let vaddr = u32le(&data, s + 12) as usize;
            let raw_off = u32le(&data, s + 20) as usize;
            if rva >= vaddr && rva < vaddr + vsize.max(1) {
                return raw_off + (rva - vaddr);
            }
        }
        rva
    };
    let exp = rva2off(exp_rva);
    let num_names = u32le(&data, exp + 24) as usize;
    let names_rva = u32le(&data, exp + 32) as usize;
    let names_off = rva2off(names_rva);
    println!("导出符号数: {}", u32le(&data, exp + 20));
    println!("命名导出数: {num_names}");
    for i in 0..num_names {
        let name_rva = u32le(&data, names_off + i * 4) as usize;
        let noff = rva2off(name_rva);
        let end = data[noff..].iter().position(|&b| b == 0).unwrap();
        println!("{}", String::from_utf8_lossy(&data[noff..noff + end]));
    }
}
