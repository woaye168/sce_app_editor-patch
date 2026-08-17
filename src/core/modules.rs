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
//! - 模块可声明 `default_enabled`：内核补丁首次创建补丁目录时自动启用。
//! - 新增内置模块：`patches/<pkg>/<id>/` 下放 lua 文件 + `builtin_modules()` 注册。

use super::{bridge_deploy, crypto};
use std::fs;
use std::path::{Path, PathBuf};

/// 注入到库 require 根下的补丁目录名（与本仓库同名）
pub const PATCH_DIR_NAME: &str = "sce_app_editor-patch";

/// 一个内置补丁模块
pub struct PatchModule {
    /// 模块 id（目录名，同时是 require 路径的一段）
    pub id: &'static str,
    /// 所属库（api_pak_version.json 包名，如 script / xdeditor）
    pub pkg: &'static str,
    /// 显示名（中文）
    pub name: &'static str,
    /// 功能描述
    pub description: &'static str,
    /// 默认勾选（内核补丁首次创建补丁目录时自动启用）
    pub default_enabled: bool,
    /// 模块文件：(模块目录内相对路径, 文件内容)
    pub files: &'static [(&'static str, &'static str)],
    /// 启用/关闭时是否同步部署/摘除 bgd_mcp_bridge.dll（引擎目录 + deps.json 登记）
    pub deploy_bridge_dll: bool,
    /// 启用时是否注入项目根（由应用把当前项目路径写为 `_project_root.lua`；
    /// 编辑器内运行时推导不可靠——如 script 包拿不到编辑器 UI 进程的真实项目路径）
    pub inject_project_root: bool,
}

/// 全部内置补丁模块（新增模块在此注册 + patches/<pkg>/<id>/ 下放文件）
pub fn builtin_modules() -> Vec<PatchModule> {
    vec![
        PatchModule {
            id: "hello",
            pkg: "script",
            name: "示例补丁",
            description: "验证补丁链路：加载时输出日志，暴露全局标记 __EDITOR_PATCH__，并报告关键函数的解禁状态。",
            default_enabled: false,
            files: &[("main.lua", include_str!("../../patches/script/hello/main.lua"))],
            deploy_bridge_dll: false,
            inject_project_root: false,
        },
        PatchModule {
            id: "unwatch",
            pkg: "xdeditor",
            name: "解除项目文件监听",
            description: "移除并拦截编辑器对项目目录的文件变更监听（io.remove_watch / io.add_watch），外部修改项目文件时不再弹出重载提示。项目根由本应用在勾选时注入（_project_root.lua）。",
            default_enabled: false,
            files: &[("main.lua", include_str!("../../patches/xdeditor/unwatch/main.lua"))],
            deploy_bridge_dll: false,
            inject_project_root: true,
        },
        PatchModule {
            id: "menu_bgd",
            pkg: "xdeditor",
            name: "帮助菜单 bgd_sce_tools 入口",
            description: "编辑器顶部菜单「帮助」下增加「bgd_sce_tools」子菜单，点击打开 bgd_sce_tools 的 GitHub 仓库。",
            default_enabled: true,
            files: &[("main.lua", include_str!("../../patches/xdeditor/menu_bgd/main.lua"))],
            deploy_bridge_dll: false,
            inject_project_root: false,
        },
        PatchModule {
            id: "bgd_mcp_bridge",
            pkg: "xdeditor",
            name: "MCP 桥（外部 AI 控制）",
            description: "在编辑器进程内启动 HTTP/MCP 服务（127.0.0.1），供外部 AI 调用编辑器命令（启动/停止调试等）。启用时部署 bgd_mcp_bridge.dll 到引擎目录。",
            default_enabled: false,
            files: &[("main.lua", include_str!("../../patches/xdeditor/bgd_mcp_bridge/main.lua"))],
            deploy_bridge_dll: true,
            inject_project_root: false,
        },
    ]
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
    let dir = patch_dir(lib_require_root).join(module.id);
    if enable {
        if module.deploy_bridge_dll {
            if let Some(vdir) = version_dir {
                bridge_deploy::deploy(vdir)?;
            }
        }
        // 先校验 project_root（需要注入的模块缺路径直接报错，不写半个模块）
        let inject_root = if enable && module.inject_project_root {
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
pub fn apply_defaults(lib_require_root: &Path, pkg: &str) -> Result<Vec<String>, String> {
    let mut applied = Vec::new();
    for module in builtin_modules()
        .iter()
        .filter(|m| m.default_enabled && m.pkg == pkg)
    {
        let dir = patch_dir(lib_require_root).join(module.id);
        if !dir.exists() {
            write_module_files(&dir, module)?;
            applied.push(module.id.to_string());
        }
    }
    regenerate_entry(lib_require_root)?;
    Ok(applied)
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
