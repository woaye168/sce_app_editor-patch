//! 内核补丁：多库补丁点的应用 / 状态检查 / 还原。
//!
//! 当前内核补丁点：
//! 1. `script/common/isolation.lua`：解锁 `xxx = nil` 禁用行 + 注入补丁框架入口
//! 2. `xdeditor/ui/menu_bar.lua`：注入「帮助/bgd_sce_tools」菜单项
//!
//! 「应用补丁」（幂等，可重复执行；编辑器升级覆盖后重新点一次即可）：
//! 读文件（加密则解密，明文则原样）→ 备份原始字节（仅首次）→ 文本转换 → 按原格式写回。
//!
//! 「还原补丁」：有备份的补丁点逐个字节级还原，并删除 common 下的补丁框架目录。

use super::locate::EditorTarget;
use super::{backup, crypto, log, modules};
use std::path::PathBuf;

/// 注入块开始/结束标记（所有补丁点共用）
pub const INJECT_BEGIN: &str = "-->> sce_app_editor-patch >>";
pub const INJECT_END: &str = "--<< sce_app_editor-patch <<";
/// 解锁行标记前缀
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 补丁点处理方式
enum PatchKind {
    /// isolation.lua：解锁禁用 + 注入框架入口
    Isolation,
    /// menu_bar.lua：注入帮助菜单项
    MenuBar,
}

/// 一个内核补丁点（某个库里的某个文件）
struct PatchPoint {
    /// 包名（api_pak_version.json 中的键）
    pkg: &'static str,
    /// 包内相对路径
    rel: &'static str,
    /// 界面显示名
    label: &'static str,
    kind: PatchKind,
}

/// 全部内核补丁点（新增内核补丁在此登记）
const PATCH_POINTS: &[PatchPoint] = &[
    PatchPoint {
        pkg: "script",
        rel: "common/isolation.lua",
        label: "解除函数禁用 + 框架入口（script/common/isolation.lua）",
        kind: PatchKind::Isolation,
    },
    PatchPoint {
        pkg: "xdeditor",
        rel: "ui/menu_bar.lua",
        label: "帮助菜单 bgd_sce_tools 入口（xdeditor/ui/menu_bar.lua）",
        kind: PatchKind::MenuBar,
    },
];

/// 单个补丁点的状态
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum FileStatus {
    /// 已应用（含注入标记）
    Applied,
    /// 未应用（原始状态，或被编辑器升级覆盖）
    NotApplied,
    /// 文件缺失或无法读取
    Missing,
}

/// 单个补丁点的完整状态（供 UI 展示）
pub struct PatchStatus {
    pub label: &'static str,
    pub path: String,
    pub status: FileStatus,
    pub has_backup: bool,
}

impl PatchPoint {
    /// 解析补丁点文件的真实路径
    fn path(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        Ok(target.package_dir(self.pkg)?.join(self.rel))
    }

    /// 备份分组 + 包内相对路径
    fn backup_key(&self, target: &EditorTarget) -> Result<(String, String), String> {
        Ok((target.backup_group(self.pkg)?, self.rel.to_string()))
    }
}

/// 检查全部补丁点状态（编辑器升级覆盖后会显示为「未应用」，重新应用即可）
pub fn check(target: &EditorTarget) -> Vec<PatchStatus> {
    PATCH_POINTS
        .iter()
        .map(|point| {
            let (path_str, status) = match point.path(target) {
                Ok(path) => {
                    let s = match crypto::read_lua(&path) {
                        Ok(lua) if lua.text.contains(INJECT_BEGIN) => FileStatus::Applied,
                        Ok(_) => FileStatus::NotApplied,
                        Err(_) => FileStatus::Missing,
                    };
                    (path.display().to_string(), s)
                }
                Err(e) => (e, FileStatus::Missing),
            };
            let has_backup = point
                .backup_key(target)
                .map(|(g, rel)| backup::has_backup(&target.editor_root, &g, &rel))
                .unwrap_or(false);
            PatchStatus {
                label: point.label,
                path: path_str,
                status,
                has_backup,
            }
        })
        .collect()
}

/// 应用全部内核补丁点，返回逐点结果摘要（全部失败才返回 Err）
pub fn apply(target: &EditorTarget) -> Result<String, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut ok_count = 0;
    for point in PATCH_POINTS {
        match apply_one(target, point) {
            Ok(msg) => {
                ok_count += 1;
                lines.push(format!("✔ {}：{msg}", point.label));
                log::log(
                    Some(&target.editor_root),
                    "INFO",
                    &format!("应用成功 [{}]: {msg}", point.rel),
                );
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", point.label));
                log::log(
                    Some(&target.editor_root),
                    "ERROR",
                    &format!("应用失败 [{}]: {e}", point.rel),
                );
            }
        }
    }
    if ok_count == 0 {
        return Err(format!("全部补丁点应用失败：\n{}", lines.join("\n")));
    }
    Ok(lines.join("\n"))
}

/// 应用单个补丁点
fn apply_one(target: &EditorTarget, point: &PatchPoint) -> Result<String, String> {
    let path = point.path(target)?;
    let lua = crypto::read_lua(&path)?;

    // 先备份再动手（仅首次真正写入）
    let (group, rel) = point.backup_key(target)?;
    let new_backup = backup::backup_file(&target.editor_root, &group, &rel, &path)?;

    // 幂等：先移除旧注入块再转换
    let base = remove_inject(&lua.text);
    let (text, extra) = match point.kind {
        PatchKind::Isolation => {
            let (text, unlocked) = transform_unlock(&base);
            (
                format!("{}\n\n{}\n", text.trim_end(), isolation_inject_block()),
                format!("解锁 {unlocked} 处禁用，注入框架入口"),
            )
        }
        PatchKind::MenuBar => (
            format!("{}\n\n{}\n", base.trim_end(), menu_inject_block()),
            "注入帮助菜单入口".to_string(),
        ),
    };
    crypto::write_lua(&path, &crypto::LuaText { text, encrypted: lua.encrypted })?;

    // isolation 补丁点附带重建补丁框架入口（保留已启用模块）
    if matches!(point.kind, PatchKind::Isolation) {
        modules::regenerate_entry(&target.common_dir()?)?;
    }

    Ok(if new_backup {
        format!("{extra}（已备份原文件）")
    } else {
        format!("{extra}（沿用已有备份）")
    })
}

/// 还原全部补丁点，返回逐点结果摘要
pub fn restore(target: &EditorTarget) -> Result<String, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut ok_count = 0;
    for point in PATCH_POINTS {
        let (group, rel) = point.backup_key(target)?;
        if !backup::has_backup(&target.editor_root, &group, &rel) {
            continue; // 没备份过 = 没改过，跳过
        }
        let path = point.path(target)?;
        match backup::restore_file(&target.editor_root, &group, &rel, &path) {
            Ok(()) => {
                ok_count += 1;
                lines.push(format!("✔ {}：已用备份还原", point.label));
                log::log(
                    Some(&target.editor_root),
                    "INFO",
                    &format!("还原成功 [{}]", point.rel),
                );
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", point.label));
                log::log(
                    Some(&target.editor_root),
                    "ERROR",
                    &format!("还原失败 [{}]: {e}", point.rel),
                );
            }
        }
    }

    // 删除补丁框架目录（含所有已启用模块）
    if let Ok(common) = target.common_dir() {
        let patch_dir = modules::patch_dir(&common);
        if patch_dir.exists() {
            std::fs::remove_dir_all(&patch_dir)
                .map_err(|e| format!("删除 {} 失败: {e}", patch_dir.display()))?;
            lines.push("✔ 已移除补丁框架目录 sce_app_editor-patch/".to_string());
        }
    }

    if ok_count == 0 {
        return Err("没有任何备份可用于还原（尚未应用过补丁）".to_string());
    }
    Ok(lines.join("\n"))
}

/// 框架入口注入块（isolation.lua 末尾）
fn isolation_inject_block() -> String {
    format!(
        "{INJECT_BEGIN}\n\
         -- 编辑器补丁框架入口（由 sce_app_editor-patch 应用注入，请勿手改）\n\
         local __ep_ok, __ep_err = pcall(require, 'sce_app_editor-patch.main')\n\
         if not __ep_ok then\n\
         \x20   log_file.info('[sce_app_editor-patch] 框架入口加载失败: ' .. tostring(__ep_err))\n\
         end\n\
         {INJECT_END}"
    )
}

/// 帮助菜单注入块（menu_bar.lua 末尾）
fn menu_inject_block() -> String {
    format!(
        "{INJECT_BEGIN}\n\
         -- 编辑器补丁：帮助菜单增加 bgd_sce_tools 入口（由 sce_app_editor-patch 应用注入，请勿手改）\n\
         window_title_bar.register('帮助/bgd_sce_tools', function(item)\n\
         \x20   common.open_url('https://github.com/woaye168/bgd_sce_tools')\n\
         end)\n\
         {INJECT_END}"
    )
}

/// 移除已有注入块（含标记行之间的所有内容）
fn remove_inject(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == INJECT_BEGIN {
            inside = true;
            continue;
        }
        if trimmed == INJECT_END {
            inside = false;
            continue;
        }
        if !inside {
            out.push(line);
        }
    }
    out.join("\n")
}

/// 解锁转换：注释掉所有 `xxx = nil` 禁用行，返回 (新文本, 解锁数量)
fn transform_unlock(text: &str) -> (String, usize) {
    let mut count = 0;
    let out = text
        .lines()
        .map(|line| {
            if is_nil_disable_line(line) {
                count += 1;
                // 保留原缩进：标记前缀插在原行前
                format!("{UNLOCK_MARK}{line}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    (out, count)
}

/// 判断是否为「标识符路径 = nil」禁用行（如 `io.popen = nil`、`_G.package.loadlib = nil`）
/// 已注释行、`local x = nil` 等不算
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::locate::locate;

    #[test]
    fn test_is_nil_disable_line() {
        assert!(is_nil_disable_line("    io.popen = nil"));
        assert!(is_nil_disable_line("os.execute = nil"));
        assert!(is_nil_disable_line("    _G.package.loadlib = nil"));
        assert!(is_nil_disable_line("cmsg_pack.set_max_pack_byte_count = nil"));
        // 已注释 / local 赋值 / 其他语句不算
        assert!(!is_nil_disable_line("-- io.unzip_file = nil"));
        assert!(!is_nil_disable_line("local x = nil"));
        assert!(!is_nil_disable_line("local write = io.write"));
        assert!(!is_nil_disable_line("io.create_dir('.')"));
        assert!(!is_nil_disable_line(""));
    }

    #[test]
    fn test_transform_unlock() {
        let src = "local a = 1\n    io.popen = nil\n    os.execute = nil\n-- io.unzip_file = nil\n";
        let (out, count) = transform_unlock(src);
        assert_eq!(count, 2);
        assert!(out.contains("-- [sce_app_editor-patch 解锁]     io.popen = nil"));
        assert!(out.contains("-- [sce_app_editor-patch 解锁]     os.execute = nil"));
        // 已注释行不重复处理
        assert!(out.contains("-- io.unzip_file = nil"));
        // 重复执行幂等
        let (_out2, count2) = transform_unlock(&out);
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_remove_inject() {
        let src = format!("line1\n{}\nabc\ndef\n{}\nline2\n", INJECT_BEGIN, INJECT_END);
        let out = remove_inject(&src);
        assert_eq!(out, "line1\nline2");
    }

    #[test]
    fn test_crypto_round_trip() {
        let plain = "hello 星火编辑器";
        let enc = crypto::encrypt(plain.as_bytes());
        assert!(crypto::is_encrypted(&enc));
        assert!(enc.starts_with(b"TNND"));
        let dec = crypto::decrypt(&enc).unwrap();
        assert_eq!(dec, plain.as_bytes());
    }

    #[test]
    fn test_plain_file_passthrough() {
        // 明文文件：读出是明文，写回仍是明文（不加密）
        let base = std::env::temp_dir().join(format!("ep_plain_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let file = base.join("plain.lua");
        std::fs::write(&file, "-- 明文 lua\nlocal a = 1\n").unwrap();

        let lua = crypto::read_lua(&file).unwrap();
        assert!(!lua.encrypted);
        crypto::write_lua(&file, &crypto::LuaText { text: format!("{}\n-- added\n", lua.text), encrypted: lua.encrypted }).unwrap();
        let raw = std::fs::read(&file).unwrap();
        assert!(!crypto::is_encrypted(&raw));
        assert!(String::from_utf8(raw).unwrap().contains("-- added"));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// 端到端：临时目录上构造双库（script 加密 + xdeditor 明文），
    /// 应用→状态→再应用（幂等）→覆盖后检测→还原 全流程
    #[test]
    fn test_apply_restore_flow() {
        let base = std::env::temp_dir().join(format!("editor_patch_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let backup_dir = base.join("backup");
        std::env::set_var("EDITOR_PATCH_BACKUP_DIR", &backup_dir);

        // 造项目结构：project/map_settings.json + script/tsconfig.json
        let project = base.join("project_x");
        std::fs::create_dir_all(project.join("project")).unwrap();
        std::fs::create_dir_all(project.join("script")).unwrap();
        std::fs::write(
            project.join("project").join("map_settings.json"),
            r#"{"api_version": {"api_version": 13}}"#,
        )
        .unwrap();
        let editor_root = base.join("editor");
        std::fs::write(
            project.join("script").join("tsconfig.json"),
            format!(
                r#"{{"compilerOptions": {{"typeRoots": ["{}"]}}}}"#,
                editor_root.display().to_string().replace('\\', "/")
                    + "/Res/_m/maps/global_default/53/global_default/script/"
            ),
        )
        .unwrap();

        // 造 api_pak_version.json + 双库文件
        std::fs::create_dir_all(&editor_root).unwrap();
        std::fs::write(
            editor_root.join("api_pak_version.json"),
            r##"{"#package_path": {"script": "Res/_m/script", "xdeditor": "Res/_m/xdeditor"},
                "13": {"script": 199, "xdeditor": 160}}"##,
        )
        .unwrap();

        // script 包：加密的 isolation.lua
        let common = editor_root.join("Res/_m/script/199/script/common");
        std::fs::create_dir_all(&common).unwrap();
        let iso_original = "local util = require 'base.util'\nif __lua_state_name == 'StateGame' then\n    io.popen = nil\n    os.execute = nil\nend\n";
        let iso = common.join("isolation.lua");
        std::fs::write(&iso, crypto::encrypt(iso_original.as_bytes())).unwrap();
        let iso_original_bytes = std::fs::read(&iso).unwrap();

        // xdeditor 包：明文的 menu_bar.lua（验证明文容错）
        let xd_ui = editor_root.join("Res/_m/xdeditor/160/xdeditor/ui");
        std::fs::create_dir_all(&xd_ui).unwrap();
        let menu_original = "window_title_bar.register('帮助/文档', function(item)\n    common.open_url('http://doc.sce.xd.com/')\nend)\n";
        let menu = xd_ui.join("menu_bar.lua");
        std::fs::write(&menu, menu_original).unwrap();

        let target = locate(&project).unwrap();

        // 应用
        let msg = apply(&target).unwrap();
        assert!(msg.contains("解锁 2 处"), "{msg}");
        assert!(msg.contains("帮助菜单"), "{msg}");
        let statuses = check(&target);
        assert!(statuses.iter().all(|s| s.status == FileStatus::Applied), "全部已应用");

        // isolation：解锁 + 框架入口
        let iso_text = crypto::read_lua(&iso).unwrap().text;
        assert!(iso_text.contains("pcall(require, 'sce_app_editor-patch.main')"));
        assert!(iso_text.contains("-- [sce_app_editor-patch 解锁]     io.popen = nil"));
        // 框架入口已生成（加密，因为新建文件默认加密？——入口是新建文件，走加密写入）
        let entry = common.join("sce_app_editor-patch").join("main.lua");
        assert!(crypto::is_encrypted(&std::fs::read(&entry).unwrap()));

        // menu_bar：明文写回仍是明文
        let menu_raw = std::fs::read(&menu).unwrap();
        assert!(!crypto::is_encrypted(&menu_raw));
        let menu_text = String::from_utf8(menu_raw).unwrap();
        assert!(menu_text.contains("window_title_bar.register('帮助/bgd_sce_tools'"));

        // 再应用：幂等
        let msg2 = apply(&target).unwrap();
        assert!(msg2.contains("解锁 0 处"), "{msg2}");

        // 模拟编辑器升级覆盖：把 isolation.lua 重置为原始（相当于被覆盖）
        std::fs::write(&iso, &iso_original_bytes).unwrap();
        let statuses = check(&target);
        let iso_status = statuses.iter().find(|s| s.label.contains("isolation")).unwrap();
        assert_eq!(iso_status.status, FileStatus::NotApplied, "覆盖后检测为未应用");
        let menu_status = statuses.iter().find(|s| s.label.contains("menu_bar")).unwrap();
        assert_eq!(menu_status.status, FileStatus::Applied, "未覆盖的仍为已应用");
        // 重新应用恢复
        apply(&target).unwrap();
        assert!(check(&target).iter().all(|s| s.status == FileStatus::Applied));

        // 还原：字节级一致
        restore(&target).unwrap();
        assert_eq!(std::fs::read(&iso).unwrap(), iso_original_bytes);
        assert_eq!(std::fs::read_to_string(&menu).unwrap(), menu_original);
        assert!(check(&target).iter().all(|s| s.status == FileStatus::NotApplied));
        assert!(!common.join("sce_app_editor-patch").exists());

        let _ = std::fs::remove_dir_all(&base);
    }
}
