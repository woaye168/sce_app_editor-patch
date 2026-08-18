//! 开发工具：生成 pie_capture 插槽文件（修复 PIE 拍照按钮，含游戏 UI + 倍率）。
//!
//! 用法：
//!   cargo run --example make_pie_slot -- <pristine的gameplay_in_editor_view.lua> <版本> <slots目录>
//!
//! pristine 文件来源：编辑器未打补丁时的官方文件（可从整库备份取：
//!   <编辑器根>/bgd_editor_patch/backup/api<api>/xdeditor_<版本>/ui/gameplay_in_editor_view.lua）
//!
//! 处理：解码（TNND/GBK 自适应）→ 替换 `bind.on_game_snapshot_click = function()...end` 整段为
//! patches/xdeditor/pie_capture/snapshot_handler.lua → 写 slots/xdeditor/<版本>/ui/ 并更新
//! slot.manifest.json（记录官方源 sha256，供内核「同内容复用」回退判定）。
//! 上游该文件结构变化导致锚点匹配失败时，本工具报错——人工核对后调整 handler 替换逻辑。

use sce_app_editor_patch::core::slot_inject;
use std::fs;
use std::path::Path;

const HANDLER: &str = include_str!("../patches/xdeditor/pie_capture/snapshot_handler.lua");
const REL: &str = "ui/gameplay_in_editor_view.lua";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("用法: make_pie_slot <pristine文件> <版本> <slots目录>");
        std::process::exit(1);
    }
    let (pristine, version, slots_root) = (&args[1], &args[2], &args[3]);

    let raw = fs::read(pristine).expect("读取 pristine 文件失败");
    let text = slot_inject::decode_source(&raw);
    let hash = slot_inject::source_hash(&text);

    // 定位 handler 块：`    bind.on_game_snapshot_click = function()` 到下一个同级 `    end`
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines
        .iter()
        .position(|l| l.trim_start().starts_with("bind.on_game_snapshot_click = function()"))
        .expect("锚点失败：找不到 bind.on_game_snapshot_click = function()（上游结构已变，需人工核对）");
    // 收尾 end 必须与起始行同缩进（handler 内有嵌套 if 的 end，trim 匹配会切错位置）
    let indent: String = lines[begin].chars().take_while(|c| c.is_whitespace()).collect();
    let end_pat = format!("{indent}end");
    let end = lines
        .iter()
        .enumerate()
        .skip(begin + 1)
        .find(|(_, l)| **l == end_pat)
        .map(|(i, _)| i)
        .expect("锚点失败：找不到 handler 的收尾 end（上游结构已变，需人工核对）");

    // 注入：handler 每行加 4 空格缩进（与原代码同级）
    let indented: Vec<String> = HANDLER
        .trim_end()
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + indented.len());
    out.extend(lines[..begin].iter().map(|s| s.to_string()));
    out.extend(indented);
    out.extend(lines[end + 1..].iter().map(|s| s.to_string()));
    let out_text = out.join("\n") + "\n";

    // 写插槽文件 + 更新 manifest
    let out_dir = Path::new(slots_root).join("xdeditor").join(version);
    let target = out_dir.join(REL);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, &out_text).unwrap();
    println!("生成 {}", target.display());

    let manifest_path = out_dir.join("slot.manifest.json");
    let mut doc: serde_json::Value = fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| {
            serde_json::json!({"pkg":"xdeditor","version":version.parse::<u64>().unwrap_or(0),"files":{}})
        });
    doc["files"][REL] = serde_json::json!({
        "kind": "pie_capture_fix",
        "source_sha256": hash,
    });
    fs::write(&manifest_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    println!("更新 {}", manifest_path.display());
}
