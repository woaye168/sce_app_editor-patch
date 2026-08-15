---
name: "sce-lib-xdeditor-160"
description: "星火编辑器 xdeditor 库（编辑器界面）v160 的源码知识库：加载流程、菜单/窗口/插件机制、事件系统、逐文件研究。补丁涉及 xdeditor 库（编辑器 UI）时查阅。"
---

# xdeditor-160 库知识库

星火编辑器 xdeditor 库（`Res/_m/xdeditor/160/xdeditor`，require 根 = 包根）v160 的源码研究成果。怎么做补丁看流程技能 `sce-editor-patch-module` / `sce-editor-lib-onboard`。

## 索引

- [architecture.md](architecture.md)：加载流程（main.lua 分支/主流程、登录回调、show_editor_main_ui）、include/require/@ 机制
- [api.md](api.md)：EDITOR/EVENT/SCE/common/base/log 等全局 API 签名
- [hooks.md](hooks.md)：已验证 hook 配方（**菜单注册事件桥**、窗口创建感知、插件机制、io hook 样板）
- [files/](files/)：逐文件研究记录（core-config / misc-modules / plugin-a / plugin-b / trigger / trigger-editor-v2 / ui / window-a/b/c，共 922 文件）
- [_plan.md](_plan.md)：研究清单

## 补丁开发速查

- **加菜单**：用 `EDITOR.event_notify(EVENT.window_title_bar_register, '一级/子菜单', callback)`（事件桥），不要在入口模块 `require 'ui.menu_bar'`（组件工厂会新建类、且时序不稳）。详见 hooks.md。
- **全局就绪点**：`EDITOR`/`EVENT`/`EDITOR.event_register` 在 main.lua:117-121（`include 'global'`/`include 'utils'`）之后可用；xdeditor 入口插槽在 main.lua 末尾顶层 return 之前，此时已就绪。
- **io hook**：编辑器 state 下可随意包装 `io.*` 全局（官方样板 io_modifier.lua）。
- **插件机制**：`plugins_manager.lua` hook 了 C++ PluginsManager.load_plugin；地图级插件走 `地图/ui/script/plugin/init.lua`。
