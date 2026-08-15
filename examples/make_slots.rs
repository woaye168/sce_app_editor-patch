//! 开发工具：生成 slots/ 插槽文件（离线，不改编辑器磁盘）。
//!
//! 用法：
//!   cargo run --example make_slots -- <编辑器根> <仓库slots目录>
//!
//! 对 kernel.rs LIBS 中的每个库 × 磁盘上存在的版本：
//!   <编辑器根>/<前缀>/<版本>/<包名>/<入口>  → slots/<库>/<版本>/<入口>（含插槽）
//!   script 库额外生成 common/isolation.lua（解锁 = nil 禁用行）
//! 处理链：解密（TNND 头则解）→ 若 UTF-8 非法则按 GBK 转 UTF-8 → 注入插槽（顶层 return 之前）。

use std::fs;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"TNND";
const KEY: [u8; 10] = *b"CREATEEASY";
const INJECT_BEGIN: &str = "-->> sce_app_editor-patch >>";
const INJECT_END: &str = "--<< sce_app_editor-patch <<";
const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// (包名, 路径前缀, 入口文件)
const LIBS: &[(&str, &str, &str)] = &[
    ("script", "Res/_m/script", "common/init.lua"),
    ("xdeditor", "Res/_m/xdeditor", "main.lua"),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: make_slots <编辑器根> <slots输出目录>");
        std::process::exit(1);
    }
    let editor_root = PathBuf::from(&args[1]);
    let slots_root = PathBuf::from(&args[2]);

    for (pkg, prefix, entry) in LIBS {
        let versions_dir = editor_root.join(prefix);
        let Ok(vers) = fs::read_dir(&versions_dir) else {
            println!("跳过 {pkg}：无目录 {}", versions_dir.display());
            continue;
        };
        for ver in vers.flatten() {
            let version = ver.file_name().to_string_lossy().to_string();
            if !version.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let pkg_dir = ver.path().join(pkg);
            if !pkg_dir.is_dir() {
                continue;
            }
            make_slot(&pkg_dir, &slots_root.join(pkg).join(&version), entry, true);
            if *pkg == "script" {
                make_slot(&pkg_dir, &slots_root.join(pkg).join(&version), "common/isolation.lua", false);
            }
        }
    }
    println!("完成 → {}", slots_root.display());
}

/// 生成一个插槽/转换文件
fn make_slot(pkg_dir: &Path, out_dir: &Path, rel: &str, inject_slot: bool) {
    let src = pkg_dir.join(rel);
    let Ok(raw) = fs::read(&src) else {
        println!("  缺失 {rel} @ {}", pkg_dir.display());
        return;
    };
    let bytes = if raw.starts_with(&MAGIC) {
        raw[4..]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ KEY[i % KEY.len()])
            .collect::<Vec<u8>>()
    } else {
        raw
    };
    let text = match String::from_utf8(bytes.clone()) {
        Ok(t) => t,
        Err(_) => {
            let (t, _, _) = encoding_rs::GBK.decode(&bytes);
            t.into_owned()
        }
    };
    let out = if inject_slot {
        insert_slot(&text, &slot_block())
    } else {
        transform_unlock(&text).0
    };
    let target = out_dir.join(rel);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, out).unwrap();
    println!("  生成 {}", target.display());
}

fn slot_block() -> String {
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

fn insert_slot(text: &str, slot: &str) -> String {
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

fn transform_unlock(text: &str) -> (String, usize) {
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
