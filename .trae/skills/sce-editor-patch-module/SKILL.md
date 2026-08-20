---
name: "sce-editor-patch-module"
description: "为星火编辑器补丁应用（sce_app_editor-patch）开发 patches/ 补丁模块（不改库源码、版本不敏感、可勾选）。当用户提出新的编辑器功能扩展/补丁需求时调用。"
---

# 开发 patches/ 补丁模块

在 `d:/sce_online/Res/maps/sce_app_editor-patch` 仓库工作。先读该仓库 `AGENTS.md` 建立全貌。

> 模块 = 不改编辑器库源码的独立补丁（运行时改行为），版本不敏感，可勾选启停。
> 需要改库源文件（含入口插槽）的场景走 slots/，用 `sce-editor-lib-onboard` 技能。

## 步骤（缺一不可）

1. **确定目标库**（模块代码运行在哪个库的 Lua 状态）：`script`（游戏脚本，require 根 `common/`）/ `xdeditor`（编辑器界面，require 根即包根）/ 其他库先接入（见 sce-editor-lib-onboard）。
2. **查库知识库**（**必须**，防幻觉）：`.trae/skills/sce-lib-<库名>-<版本>/`——先看 `hooks.md` 有没有现成配方，再查 `architecture.md`/`api.md` 确认机制与全局可用性。没有对应版本知识库就先研究建立。
3. **写模块文件**：`patches/<pkg>/<模块id>/main.lua`。约束：
   - 模块在框架入口 `pcall(require, ...)` 时执行，`return M` 收尾
   - **防御式访问**：编辑器 API 用 `pcall`/nil 判断包裹；失败经 `log`/`log_file`（判空）输出，绝不抛异常拖垮框架入口
   - 注册类补丁优先用官方**事件桥/回调**（如菜单用 `EDITOR.event_notify(EVENT.window_title_bar_register, ...)`），不要在不当时机 require 未加载的模块
   - 需要延迟执行时挂官方事件（如 `EVENT.load_map_done`）再执行，同时立即尝试一次（已加载则立即生效）
4. **注册模块**：`src/core/modules.rs` 的 `builtin_modules()` 加 `PatchModule { id, pkg, name, description, default_enabled, files, deploy_bridge_dll, inject_project_root }`；`files` 用 `include_str!("../../patches/<pkg>/<id>/main.lua")`；`default_enabled: true` 仅给必要补丁；`inject_project_root: true` 表示勾选时应用把当前项目根写入模块目录 `_project_root.lua`（模块内 `require('sce_app_editor-patch.<id>._project_root')` 取用）
5. **验证**：`cargo test` 全过 → `cargo build --release`
6. **文档同步**：AGENTS.md「现有模块」+ README 功能列表，同次提交
7. **发版**：`git tag v0.x.y && git push origin v0.x.y`（CI 出包）→ bump `d:/sce_online/Res/maps/bgd_sce_appsdk/registry.json` 的 `version`/`tag` 并推送

## 现有模块参考

- `patches/xdeditor/menu_bgd/`：**菜单注册标准范例**（事件桥 + load_map_done 延迟 + 防御判空）
- `patches/xdeditor/unwatch/`：io 函数包装 + `inject_project_root` 项目根注入范例（应用勾选时写 `_project_root.lua`）
- `patches/script/hello/`：最小模块骨架

## 关键背景（速查，细节以库知识库为准）

- 模块文件由本应用写入库 require 根下 `sce_app_editor-patch/<id>/`，一律明文
- 启用状态 = 文件系统状态（目录存在即启用）
- require 路径以库 require 根为起点；跨库用 `@` 前缀
- script 库插槽每个 lua state 各执行一次，state 相关逻辑用 `__lua_state_name` 判断
- 编辑器 state（StateEditor）下 io/os/debug 完整；StateGame 下被 isolation 阉割（内核补丁已解锁）

## 安全红线

- 模块只写自己库的 `sce_app_editor-patch/<id>/` 目录；**改编辑器源文件属于 slots/ 范畴**，别在模块里做
- Conventional Commits 提交（`feat:`/`fix:`）
