//! 补丁模块管理（按库分组）。
//!
//! 每个补丁模块归属于一个库（`pkg`），启用后写入该库 require 根下的补丁目录：
//!
//! ```text
//! <库require根>/sce_app_editor-patch/
//!   main.lua            # 框架入口（AUTO-GENERATED，按启用列表重建）
//!   <模块id>/main.lua   # 模块文件
//! ```
//!
//! - 库经内核补丁整库解密后为裸露源码，框架/模块文件也写明文（便于查看调试）。
//! - **启用状态即文件系统状态**：模块目录存在即启用，无额外状态文件。
//! - **模块元数据外置（0.5.3 起）**：`patches/modules.json` 声明各模块
//!   id/pkg/名称/描述/默认勾选/部署dll/注入项目根，编译期 include_str! 嵌入；
//!   调整默认勾选只改该文件。模块 lua 文件仍在 `patches/<pkg>/<id>/` 下，
//!   由 `module_files` 按 id 挂载（include_str! 嵌入）。
//! - 模块可声明 `default_enabled`：内核补丁首次创建补丁目录时自动启用。

use super::{bridge_deploy, crypto};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// 注入到库 require 根下的补丁目录名（与本仓库同名）
pub const PATCH_DIR_NAME: &str = "sce_app_editor-patch";

/// 编译期嵌入的模块清单（patches/modules.json）
const MODULES_JSON: &str = include_str!("../../patches/modules.json");

#[derive(Deserialize)]
struct ModulesFile {
    modules: Vec<ModuleMeta>,
}

#[derive(Deserialize)]
struct ModuleMeta {
    id: String,
    pkg: String,
    name: String,
    description: String,
    #[serde(default)]
    default_enabled: bool,
    #[serde(default)]
    deploy_bridge_dll: bool,
    #[serde(default)]
    inject_project_root: bool,
    /// 启用时注入本应用 exe 路径（`_exe_path.lua`；如 pie_capture 的拍照按钮要回调 capture CLI）
    #[serde(default)]
    inject_exe_path: bool,
}

/// 一个内置补丁模块
pub struct PatchModule {
    /// 模块 id（目录名，同时是 require 路径的一段）
    pub id: String,
    /// 所属库（api_pak_version.json 包名，如 script / xdeditor）
    pub pkg: String,
    /// 显示名（中文）
    pub name: String,
    /// 功能描述
    pub description: String,
    /// 默认勾选（内核补丁首次创建补丁目录时自动启用）
    pub default_enabled: bool,
    /// 模块文件：(模块目录内相对路径, 文件内容)
    pub files: &'static [(&'static str, &'static str)],
    /// 启用/关闭时是否同步部署/摘除 bgd_mcp_bridge.dll（引擎目录 + deps.json 登记）
    pub deploy_bridge_dll: bool,
    /// 启用时是否注入项目根（由应用把当前项目路径写为 `_project_root.lua`；
    /// 编辑器内运行时推导不可靠——如 script 包拿不到编辑器 UI 进程的真实项目路径）
    pub inject_project_root: bool,
    /// 启用时是否注入本应用 exe 路径（`_exe_path.lua`）
    pub inject_exe_path: bool,
}

/// 模块 lua 文件（编译期 include_str! 嵌入）。新增模块在此挂载。
fn module_files(id: &str) -> &'static [(&'static str, &'static str)] {
    match id {
        "hello" => &[("main.lua", include_str!("../../patches/script/hello/main.lua"))],
        "unwatch" => &[("main.lua", include_str!("../../patches/xdeditor/unwatch/main.lua"))],
        "menu_bgd" => &[("main.lua", include_str!("../../patches/xdeditor/menu_bgd/main.lua"))],
        "bgd_mcp_bridge" => &[(
            "main.lua",
            include_str!("../../patches/xdeditor/bgd_mcp_bridge/main.lua"),
        )],
        "pie_capture" => &[(
            "main.lua",
            include_str!("../../patches/xdeditor/pie_capture/main.lua"),
        )],
        _ => &[],
    }
}

/// 全部内置补丁模块（读 patches/modules.json + module_files 挂文件）
pub fn builtin_modules() -> Vec<PatchModule> {
    let parsed: ModulesFile =
        serde_json::from_str(MODULES_JSON).expect("patches/modules.json 解析失败");
    parsed
        .modules
        .into_iter()
        .map(|m| PatchModule {
            files: module_files(&m.id),
            id: m.id,
            pkg: m.pkg,
            name: m.name,
            description: m.description,
            default_enabled: m.default_enabled,
            deploy_bridge_dll: m.deploy_bridge_dll,
            inject_project_root: m.inject_project_root,
            inject_exe_path: m.inject_exe_path,
        })
        .collect()
}

/// 指定库的补丁目录：<库require根>/sce_app_editor-patch
pub fn patch_dir(lib_require_root: &Path) -> PathBuf {
    lib_require_root.join(PATCH_DIR_NAME)
}

/// 当前已启用的模块 id 列表（扫描补丁目录下含 main.lua 的子目录）
pub fn enabled_modules(lib_require_root: &Path) -> Vec<String> {
    let mut ids = Vec::new();
    let dir = patch_dir(lib_require_root);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("main.lua").is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    ids.push(name.to_string());
                }
            }
        }
    }
    ids.sort();
    ids
}

/// 启用/关闭一个模块，并重建该库框架入口。
/// `version_dir`：引擎版本目录（<运行根>/version-<api>），模块声明 `deploy_bridge_dll`
/// 时用于部署/摘除 bgd_mcp_bridge.dll；部署失败直接报错且不写模块文件，
/// 避免 Lua 模块加载了但 dll 不在。
/// `project_root`：当前项目根目录，模块声明 `inject_project_root` 时必填
/// （启用时写 `_project_root.lua` 注入；缺失直接报错，不写半个模块）。
pub fn set_module(
    lib_require_root: &Path,
    module: &PatchModule,
    enable: bool,
    version_dir: Option<&Path>,
    project_root: Option<&Path>,
) -> Result<(), String> {
    let dir = patch_dir(lib_require_root).join(&module.id);
    if enable {
        if module.deploy_bridge_dll {
            if let Some(vdir) = version_dir {
                bridge_deploy::deploy(vdir)?;
            }
        }
        // 先校验 project_root（需要注入的模块缺路径直接报错，不写半个模块）
        let inject_root = if module.inject_project_root {
            Some(project_root.ok_or_else(|| {
                format!("模块「{}」需要项目路径注入（请先在应用内选择/确认项目）", module.name)
            })?)
        } else {
            None
        };
        write_module_files(&dir, module)?;
        if let Some(root) = inject_root {
            write_project_root(&dir, root)?;
        }
        if module.inject_exe_path {
            write_exe_path(&dir)?;
        }
    } else {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| format!("删除 {} 失败: {e}", dir.display()))?;
        }
        if module.deploy_bridge_dll {
            if let Some(vdir) = version_dir {
                bridge_deploy::undeploy(vdir)?;
            }
        }
    }
    regenerate_entry(lib_require_root)
}

/// 同步注入的项目根（F1：项目切换后自动刷新）。
/// 读取已启用模块的 `_project_root.lua`，与当前项目根比对，不一致则原子重写。
/// 返回是否发生了重写；模块未启用/无注入文件时返回 Ok(false)。
pub fn sync_project_root(
    lib_require_root: &Path,
    module: &PatchModule,
    project_root: &Path,
) -> Result<bool, String> {
    if !module.inject_project_root {
        return Ok(false);
    }
    let file = patch_dir(lib_require_root)
        .join(&module.id)
        .join("_project_root.lua");
    sync_injected_file(&file, &project_root.to_string_lossy().replace('\\', "/"))
}

/// 同步已启用模块的部署文件（0.5.7：应用升级后自动更新模块内容）。
/// 逐文件比对嵌入内容与部署内容，不一致则原子重写；注入文件（_project_root/_exe_path）
/// 由各自 sync 处理，不在此列。返回是否有文件被重写。
pub fn sync_module_files(lib_require_root: &Path, module: &PatchModule) -> Result<bool, String> {
    let dir = patch_dir(lib_require_root).join(&module.id);
    if !dir.is_dir() {
        return Ok(false); // 未启用
    }
    let mut changed = false;
    for (name, content) in module.files {
        let file = dir.join(name);
        let deployed = fs::read(&file).unwrap_or_default();
        if deployed != content.as_bytes() {
            crypto::write_atomic(&file, content.as_bytes())?;
            changed = true;
        }
    }
    Ok(changed)
}

/// 同步注入的 exe 路径（应用位置变化后自动刷新；`_exe_path.lua`）
pub fn sync_exe_path(lib_require_root: &Path, module: &PatchModule) -> Result<bool, String> {
    if !module.inject_exe_path {
        return Ok(false);
    }
    let Some(exe) = std::env::current_exe().ok() else {
        return Ok(false);
    };
    let file = patch_dir(lib_require_root)
        .join(&module.id)
        .join("_exe_path.lua");
    sync_injected_file(&file, &exe.to_string_lossy().replace('\\', "/"))
}

/// 注入文件内容比对 + 不一致原子重写（模块未启用/无注入文件返回 Ok(false)）
fn sync_injected_file(file: &Path, expected: &str) -> Result<bool, String> {
    if !file.is_file() {
        return Ok(false);
    }
    let existing = fs::read_to_string(file).map_err(|e| format!("读取 {} 失败: {e}", file.display()))?;
    let injected = existing
        .lines()
        .find_map(|l| l.trim().strip_prefix("return [[").and_then(|s| s.strip_suffix("]]")))
        .unwrap_or("");
    if injected == expected {
        return Ok(false);
    }
    if expected.contains("]]" ) {
        return Err("注入内容含非法字符 ]]".into());
    }
    let content = format!(
        "-- AUTO-GENERATED by sce_app_editor-patch（启用模块时注入），请勿手改\nreturn [[{expected}]]\n"
    );
    crypto::write_atomic(file, content.as_bytes())?;
    Ok(true)
}

/// 注入项目根：写 `_project_root.lua`（统一为正斜杠，Lua 侧长括号字符串免转义）
fn write_project_root(dir: &Path, project_root: &Path) -> Result<(), String> {
    let normalized = project_root.to_string_lossy().replace('\\', "/");
    if normalized.contains("]]") {
        return Err("项目路径含非法字符 ]] ".into());
    }
    let content = format!(
        "-- AUTO-GENERATED by sce_app_editor-patch（启用模块时注入当前项目根），请勿手改\nreturn [[{normalized}]]\n"
    );
    crypto::write_atomic(&dir.join("_project_root.lua"), content.as_bytes())
}

/// 注入本应用 exe 路径：写 `_exe_path.lua`（pie_capture 拍照按钮回调 capture CLI 用）
fn write_exe_path(dir: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取应用 exe 路径失败: {e}"))?;
    let normalized = exe.to_string_lossy().replace('\\', "/");
    if normalized.contains("]]") {
        return Err("exe 路径含非法字符 ]] ".into());
    }
    let content = format!(
        "-- AUTO-GENERATED by sce_app_editor-patch（启用模块时注入应用 exe 路径），请勿手改\nreturn [[{normalized}]]\n"
    );
    crypto::write_atomic(&dir.join("_exe_path.lua"), content.as_bytes())
}

/// 写出一个模块的全部文件（明文）
fn write_module_files(dir: &Path, module: &PatchModule) -> Result<(), String> {
    for (rel, content) in module.files {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        crypto::write_atomic(&path, content.as_bytes())?;
    }
    Ok(())
}

/// 启用所有默认勾选的模块（内核补丁首次创建补丁目录时调用）。
/// 必须按 `pkg` 过滤——本函数对每个库各调一次，不过滤会把 xdeditor 的默认模块写进 script 包。
/// `version_dir`/`project_root`：默认模块声明 deploy_bridge_dll / inject_project_root 时使用
/// （0.5.3 起 bgd_mcp_bridge/unwatch 默认勾选，内核应用路径必须具备完整启用能力）。
/// 单个默认模块启用失败（如编辑器运行中锁定 dll）不中断整体应用，失败项并入返回的 warnings。
/// 返回 (已启用列表, 警告列表)；仅入口重建失败才返回 Err。
pub fn apply_defaults(
    lib_require_root: &Path,
    pkg: &str,
    version_dir: Option<&Path>,
    project_root: Option<&Path>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut applied = Vec::new();
    let mut warnings = Vec::new();
    for module in builtin_modules()
        .iter()
        .filter(|m| m.default_enabled && m.pkg == pkg)
    {
        let dir = patch_dir(lib_require_root).join(&module.id);
        if dir.exists() {
            continue;
        }
        match set_module(lib_require_root, module, true, version_dir, project_root) {
            Ok(()) => applied.push(module.id.clone()),
            Err(e) => warnings.push(format!("默认模块[{}]启用失败: {e}", module.id)),
        }
    }
    regenerate_entry(lib_require_root)?;
    Ok((applied, warnings))
}

/// 按当前启用列表重建框架入口 main.lua（AUTO-GENERATED，明文写入）
pub fn regenerate_entry(lib_require_root: &Path) -> Result<(), String> {
    let dir = patch_dir(lib_require_root);
    fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;

    let ids = enabled_modules(lib_require_root);
    let mut text = String::from(
        "-- AUTO-GENERATED by sce_app_editor-patch（编辑器补丁），请勿手改\n\
         -- 本文件按已启用的补丁模块列表重建\n\
         local modules = {",
    );
    for id in &ids {
        text.push_str(&format!(" '{id}',"));
    }
    text.push_str(" }\n");
    text.push_str(
        "for _, id in ipairs(modules) do\n\
         \x20   local ok, err = pcall(require, 'sce_app_editor-patch.' .. id .. '.main')\n\
         \x20   if not ok and log_file and log_file.info then\n\
         \x20       log_file.info('[sce_app_editor-patch] 模块[' .. id .. ']加载失败: ' .. tostring(err))\n\
         \x20   end\n\
         end\n",
    );

    crypto::write_atomic(&dir.join("main.lua"), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_module<'a>(files: &'static [(&'static str, &'static str)]) -> PatchModule {
        PatchModule {
            id: "testmod".into(),
            pkg: "xdeditor".into(),
            name: "测试模块".into(),
            description: String::new(),
            default_enabled: false,
            files,
            deploy_bridge_dll: false,
            inject_project_root: false,
            inject_exe_path: false,
        }
    }

    /// 0.5.7：升级后已启用模块的部署文件按嵌入内容自动刷新
    #[test]
    fn test_sync_module_files() {
        let dir = std::env::temp_dir().join(format!("bgd_sync_mod_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let lib_root = dir.join("lib");
        let mod_dir = lib_root.join("sce_app_editor-patch").join("testmod");
        fs::create_dir_all(&mod_dir).unwrap();
        // 已部署旧版内容（v1），嵌入内容已是 v2
        fs::write(mod_dir.join("main.lua"), "-- v1 old").unwrap();
        static FILES: &[(&str, &str)] = &[("main.lua", "-- v2 new")];
        let m = test_module(FILES);

        // 未启用（目录不存在）→ false
        let missing_root = dir.join("missing");
        assert!(!sync_module_files(&missing_root, &m).unwrap());

        // 内容不一致 → 重写并返回 true
        assert!(sync_module_files(&lib_root, &m).unwrap());
        assert_eq!(fs::read_to_string(mod_dir.join("main.lua")).unwrap(), "-- v2 new");

        // 已一致 → false 不再写
        assert!(!sync_module_files(&lib_root, &m).unwrap());

        let _ = fs::remove_dir_all(&dir);
    }
}
