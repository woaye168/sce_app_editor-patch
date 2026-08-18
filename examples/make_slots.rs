//! 开发工具：生成 slots/ 插槽文件（离线，不改编辑器磁盘）。
//!
//! 用法：
//!   cargo run --example make_slots -- <编辑器根> <仓库slots目录>
//!
//! 对内核 LIBS 登记的每个库 × 磁盘上存在的版本：
//!   <编辑器根>/<前缀>/<版本>/<包名>/<入口>  → slots/<库>/<版本>/<入口>（含插槽）
//!   script 库额外生成 common/isolation.lua（解锁 = nil 禁用行）
//! 处理链（与内核运行时注入共用 slot_inject 实现，单一事实源）：
//!   解密（TNND 头则解）→ 若 UTF-8 非法则按 GBK 转 UTF-8 → 注入插槽（顶层 return 之前）
//! 0.5.3 起同时生成 slot.manifest.json（记录每个插槽文件的官方源 sha256，
//! 供内核「同内容复用」回退判定：新版本解码源哈希一致即可直接复用本版本插槽）。

use sce_app_editor_patch::core::slot_inject;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// (包名, 路径前缀, 入口文件)
const LIBS: &[(&str, &str, &str)] = &[
    ("script", "Res/_m/script", "common/init.lua"),
    ("xdeditor", "Res/_m/xdeditor", "main.lua"),
];

/// manifest 中的一个文件条目
struct ManifestEntry {
    kind: &'static str,
    source_sha256: String,
}

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
            let out_dir = slots_root.join(pkg).join(&version);
            let mut manifest: BTreeMap<String, ManifestEntry> = BTreeMap::new();
            make_slot(&pkg_dir, &out_dir, entry, "slot", &mut manifest);
            if *pkg == "script" {
                make_slot(&pkg_dir, &out_dir, "common/isolation.lua", "unlock", &mut manifest);
            }
            write_manifest(&out_dir, pkg, &version, &manifest);
        }
    }
    println!("完成 → {}", slots_root.display());
}

/// 生成一个插槽/转换文件，并把官方源哈希记入 manifest
fn make_slot(
    pkg_dir: &Path,
    out_dir: &Path,
    rel: &str,
    kind: &'static str,
    manifest: &mut BTreeMap<String, ManifestEntry>,
) {
    let src = pkg_dir.join(rel);
    let Ok(raw) = fs::read(&src) else {
        println!("  缺失 {rel} @ {}", pkg_dir.display());
        return;
    };
    let text = slot_inject::decode_source(&raw);
    // 幂等：编辑器可能已打补丁（源里已含插槽/解锁标记），先剥离再重注入，
    // manifest 哈希也必须是「干净官方源」的哈希（运行时复用判定比对的是未打补丁的新版本源）
    let clean = match kind {
        "slot" => slot_inject::strip_slot(&text),
        _ => slot_inject::strip_unlock(&text),
    };
    let source_sha256 = slot_inject::source_hash(&clean);
    let out = match kind {
        "slot" => slot_inject::insert_slot(&clean, &slot_inject::slot_block()),
        _ => slot_inject::transform_unlock(&clean).0,
    };
    let target = out_dir.join(rel);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, out).unwrap();
    manifest.insert(
        rel.to_string(),
        ManifestEntry { kind, source_sha256 },
    );
    println!("  生成 {}", target.display());
}

/// 写出 slot.manifest.json（内核复用判定的依据）
fn write_manifest(out_dir: &Path, pkg: &str, version: &str, manifest: &BTreeMap<String, ManifestEntry>) {
    if manifest.is_empty() {
        return;
    }
    let files: serde_json::Map<String, serde_json::Value> = manifest
        .iter()
        .map(|(rel, e)| {
            (
                rel.clone(),
                serde_json::json!({ "kind": e.kind, "source_sha256": e.source_sha256 }),
            )
        })
        .collect();
    let doc = serde_json::json!({
        "pkg": pkg,
        "version": version.parse::<u64>().unwrap_or(0),
        "files": files,
    });
    let path = out_dir.join("slot.manifest.json");
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    println!("  生成 {}", path.display());
}
