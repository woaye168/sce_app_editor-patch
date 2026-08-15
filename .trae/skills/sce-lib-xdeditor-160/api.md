# xdeditor-160 关键全局与 API

> 签名实证见 files/ 文档行号。

## 全局（main.lua:117-121 初始化链后就绪）

| 全局 | 来源 | 要点 |
|---|---|---|
| `EDITOR` | global/global.lua:2 | 编辑器主表；`EDITOR.event_register/event_notify/register_componet_event/notify_componet_event` 由 utils/event.lua:804-807 挂入；`EDITOR.utils.*`、`EDITOR.save_map`、`EDITOR.guide_config` 等 |
| `EVENT` | global/global.lua:7-211 | 事件常量表（~200 个）：`load_map`/`load_map_done`/`save_map`/`window_title_bar_register`/`window_title_bar_unregister`/`obj_init`/… |
| `CPP_EVENT` | global/global.lua:212 | C++ 侧事件常量 |
| `SCE` | `ImportSCEContext()` | C++ 上下文：`SCE.GetEventManager()`、`SCE.GetMainWindow()`、`SCE.GetPluginsManager()`、`SCE.Common.create_csharp_module(name)`、`SCE.MAPINFO.*`、`SCE.GetProjectSettings()` |
| `ProcessInfo` | sub_process_enter_point/init_process_info（main.lua:124） | `ProcessInfo.is_main_process`、`ProcessInfo.MainProcess/SubProcess` |
| `eventMgr` | `SCE.GetEventManager()` | `eventMgr:register_event('EditorMainTitleMenuBar', fn)`（C#→Lua 菜单点击入口） |
| `log` / `common` / `base` / `argv` | script 库/api.md 同款 | 编辑器 state 下功能完整 |

## 菜单系统（window_title_bar）

| API | 位置 | 签名/说明 |
|---|---|---|
| `window_title_bar.register` | ui/menu_bar.lua:1100 | `(name, callback, key=nil, guide_register=nil, process_type=nil)`；name 用 `一级/子菜单` 分层 |
| `window_title_bar.unregister` | ui/menu_bar.lua:1126 | `(name, hide)` |
| `window_title_bar.register_command / call_command` | ui/menu_bar.lua:1056/1060 | name → callback 映射表 callback_map |
| 点击链路 | ui/menu_bar.lua:1066-1069 | C# 触发 `EditorMainTitleMenuBar` 事件 → `call_command(name)` |
| 事件桥 | ui/menu_bar.lua:1134-1139 | `EDITOR.event_register(EVENT.window_title_bar_register, function(_, ...) window_title_bar.register(...) end)`（unregister 同理） |

## 窗口框架

- `WindowApp`（window/window_app.lua:10，`SCE.FWindow`）：所有窗口 app 基类；构造经 `base.ui.create_ui_root`（:27）+ `_G.WINDOW_APP_MANAGER:handle_window`（:28，win_app_manager.lua:162 全局单例）——感知全编辑器窗口创建的两个 hook 点
- `_G.trigger_editor_on`：触发编辑器全家（V1/V2）总开关（window/init.lua:8）

## 插件机制

- `plugins_manager.lua:9-10`：保存并包装 C++ `PluginsManager.load_plugin/unload_plugin`，Lua 插件 UI 生命周期挂在这层 hook 上
- Lua 插件写法：`class(name, SCE.Plugin)` + `register_plugin(name, Class)`（官方样例 plugin/sample/sample_plugin.lua）
- 地图级插件：`plugins_manager.lua:446-503`，地图目录 `ui/script/plugin/init.lua` 提供 load/unload
