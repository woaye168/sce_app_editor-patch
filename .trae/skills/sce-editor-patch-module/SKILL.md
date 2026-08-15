---
name: "sce-editor-patch-module"
description: "为星火编辑器补丁应用（sce_app_editor-patch）新增补丁模块。当用户提出新的编辑器功能扩展/补丁需求（改编辑器界面、解除限制、增强脚本能力等）时调用。"
---

# 新增编辑器补丁模块

在 `d:\sce_online\Res\maps\sce_app_editor-patch` 仓库工作。先读该仓库 `AGENTS.md` 建立全貌，再按本技能执行。

## 模块是什么

补丁模块 = 一段 Lua 代码，内核补丁应用后被注入到目标编辑器库的 require 根下，由框架入口 `sce_app_editor-patch/main.lua` 按启用列表 `pcall(require, ...)` 加载。用户在应用「补丁」标签页勾选启停。

## 新增模块步骤（缺一不可）

1. **确定目标库**（模块代码运行在哪个库的 Lua 状态）：
   - `script`：游戏脚本/common 包（StateGame 等游戏态），require 根 = 包内 `common/`
   - `xdeditor`：编辑器界面库，require 根 = 包根
   - 其他库需先接入（用 `sce-editor-lib-onboard` 技能）
2. **写模块文件**：`patches/<pkg>/<模块id>/main.lua`（多文件则同目录放置）。约束：
   - 模块在被加载时执行（库入口插槽 → 框架入口 → pcall require），`return M` 收尾
   - **防御式访问编辑器 API**：目标对象可能不存在或未加载，用 `pcall` / nil 判断包裹，失败用 `log_file.info`（判空后）输出，绝不能让模块抛异常拖垮框架入口
   - 依赖编辑器内部模块时用 `require '<库内路径>'` 拿到的是**模块缓存单例**（如 `require 'ui.menu_bar'` 拿到的就是 window_title_bar 实例），可直接改
3. **注册模块**：`src/core/modules.rs` 的 `builtin_modules()` 加一条 `PatchModule { id, pkg, name, description, default_enabled, files }`：
   - `files` 用 `include_str!("../../patches/<pkg>/<id>/main.lua")`
   - `default_enabled: true` 仅给「必要补丁」（内核首次创建补丁目录时自动启用）
4. **验证**：`cargo test` 全过 → `cargo build --release` → 必要时更新 kernel.rs 端到端测试断言
5. **文档同步**：AGENTS.md「现有模块」清单 + README 功能列表，与代码同次提交
6. **发版**：`git tag v0.x.y && git push origin v0.x.y`（CI 自动出包），然后 bump `d:\sce_online\Res\maps\bgd_sce_plugins\registry.json` 的 `version`/`tag` 并推送

## 关键背景（防幻觉）

- 编辑器包 lua 加密格式：4 字节 magic `TNND` + 密钥 `CREATEEASY` 循环异或；内核应用补丁后整库已是明文
- 模块文件由本应用写入，一律明文（`crypto::write_atomic`）
- 启用状态 = 文件系统状态（模块目录存在即启用），无状态文件
- 库内 require 路径以 require 根为起点：script 库 `require 'base.path'` 解析 `common/base/path.lua`；xdeditor 库 `require 'ui.menu_bar'` 解析 `xdeditor/ui/menu_bar.lua`；跨库引用用 `@` 前缀（如 `require '@common.base.util'`）
- script 库补丁点在 StateGame 态才有意义（isolation 只在该态阉割函数）
- 常用可用全局（视库而定，务必判空）：`log_file`、`common`（open_url 等）、`base`、`io`/`os`/`debug`（script 库解锁后恢复）

## 安全红线

- 模块开发不直接改编辑器源文件——模块只往自己库的 `sce_app_editor-patch/<id>/` 目录写；改编辑器文件属于内核补丁点范畴，必须先备份（kernel.rs 已内置）
- 提交信息用 Conventional Commits（`feat:`/`fix:` 前缀，Release notes 依赖）
