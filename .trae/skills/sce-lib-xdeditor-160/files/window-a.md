# xdeditor-160 / window 根级文件逐文件研究记录（批次 A）

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/window/` 根级 31 个 .lua。
> window/ 拆三批：**A=根级文件（本文件，app 框架详写）；B=art_workbench/ 子目录（window-b.md）；C=其余子目录 autotest/components_lib/info_window/mechanism_editor/project_manager/refer_window/resource_store/ui（window-c.md）**。
> 全部结论来自真实读取，关键结论标注行号。

## App 框架机制总结（菜单注册 + 窗口创建/显示）

### 1. WindowApp 基类（window/window_app.lua）

- `WindowApp = class('WindowApp', SCE.FWindow)`（window_app.lua:10）——**所有编辑器窗口 app 的基类，直接继承 C++ FWindow**。
- 构造（:22-30）：`base.ui.create_ui_root(self, self.id)`（:27，即 ui/window_ui.lua:3 打的补丁函数）创建 UI 根；`_G.WINDOW_APP_MANAGER:handle_window(self)`（:28）登记到全局窗口管理器。
- 典型创建：`window_app.new('TriggerEditor', 100, 100, w, h, false, '触发编辑器')`（trigger_editor_app.lua:1637）——参数 `(id, x, y, w, h, show, title)`；`WindowApp.__create`（:12-18）特判首参 string 时转发给 C++ 构造。
- 生命周期：`destroy()`（:41-51，destroy_ui_root + WINDOW_APP_MANAGER:remove_handle + base.next 后 `__release`）；`close()`（:60-74，触发 on_close 后 set_visible(false)+destroy）；`hide()/show()`（:82-116）。
- 自研事件：`on_close/on_hide/on_show/register_event/remove_event/send_event`（:76-156），`self.events` 表。
- 头注释（:1-7）明确：lua gc 目前不会回收窗口，必须主动 destroy。

### 2. 全局窗口管理器 WINDOW_APP_MANAGER（window/art_workbench/window_manager/win_app_manager.lua）

- `_G.WINDOW_APP_MANAGER = win_app_manager.new()`（win_app_manager.lua:162）——**require 本文件即创建全局单例**（window/init.lua:12 与 trigger_editor_app.lua:1629 均为此目的 require 它）。
- `create_window(props)`（:26-47）：防重名、居中、modal；`close_all()`（:53-57，menu_bar.lua:1005 退出编辑器时调用）；`handle_window/remove_handle`（:148-156）。

### 3. 菜单注册两种模式（实证）

| 模式 | 样例 | 说明 |
| --- | --- | --- |
| `menu_bar.register(name, cb, key, guide, process_type)` 直调 | backend_script_app.lua:883-902、gui_editor_app.lua:62、object_editor_app.lua:41、mechanism_editor_app.lua:88、random_terrain_config_app.lua:486、components_lib/init.lua:28 | `menu_bar = require 'ui.menu_bar'` 拿到 window_title_bar 单例直接注册 |
| `EDITOR.event_notify(EVENT.window_title_bar_register, ...)` | trigger_editor_app.lua:1660 | 走事件总线，由 menu_bar.lua:1134-1136 的 handler 落 register |

两种模式最终都进 `window_title_bar.register`（menu_bar.lua:1100）。菜单名为 `'模块/触发编辑器'` 式层级路径。

### 4. App 类第二种风格（class 单例 + window/init.lua 实例化）

profiler/texture_viewer/texture_merger/resources_manager/project_manager/mechanism_editor 等采用 `class('XxxApp')` + `self.window = nil` + `:show()` 内 `WindowApp.new(...)` 懒创建（如 texture_viewer_app.lua:3、:11-12；resources_manager_app.lua:2、:5-7），再由 **window/init.lua:28-40 统一 `XxxAppClass.new()` 实例化为 `_G.PROFILER_APP` 等全局**。window/init.lua 同时负责：require 各 app 模块（:3-7）、`_G.trigger_editor_on == true` 时才 `require 'trigger.entry'`（:8-10）、require win_app_manager 与 art_workbench 组件（:12-13）、inner 才加载 components_lib（:14-16）。

---

## window/window_app.lua
- 用途：窗口 App 基类（继承 SCE.FWindow），UI 根创建、生命周期、自研事件。
- 导出：`WindowApp`（:158）。
- 依赖：`ImportSCEContext()`（:9）、`base.ui.create_ui_root/destroy_ui_root`（ui/window_ui.lua 提供）、`_G.WINDOW_APP_MANAGER`（win_app_manager.lua:162 提供）。
- 补丁相关：**所有 app 窗口的统一构造点**——hook `WindowApp.ctor`（:22）或 `WINDOW_APP_MANAGER:handle_window`（:28）可感知全部窗口创建；`base.ui.create_ui_root`（:27）是 UI 根注入点。

## window/init.lua
- 用途：window 层引导：require 各 app 模块 + 实例化 app 全局单例。
- 导出：无（执行型）。
- 依赖：见上方机制总结 4。
- 补丁相关：**关键加载时机**——`_G.RESOURCES_MANAGER_APP / RESOURCE_STORE_APP / ART_WORKBENCH_APP / ART_WORKBENCH_APP_WinUI / ANIM_EDITOR_APP / PHYSICS_EDITOR_APP / SKELETON_EDITOR_APP / RETARGET_EDITOR_APP / PROFILER_APP / MECHANISM_MANAGER / PHYSICS_ANIM_EDITOR_APP / TEXTURE_VIEWER_APP / TEXTURE_MERGER_APP`（:28-40）全部在此建立；`trigger.entry` 的加载受 `_G.trigger_editor_on` 门控（:8-10）。

## window/trigger_editor_app.lua
- 用途：V1 触发编辑器 app（2347 行）：左树（依赖库/触发文件树）+ 画布组件 + 搜索 + 调试面板整合。
- 导出：`{ save_code, save_script_tree_config, show, get_trigger_editor_component_instance }`（:2347-2352）。
- 依赖：`window.window_app`（:9）、`trigger.trigger_editor_ui`（:10）、`trigger.ui.message_box_ui / search_box_ui`（:11-12）、`ui.components.window_title`（:13）、`ui.components.tree_with_search_new`（:14）、`ini.object_tree.node_type`（:15）、`trigger.trigger_manager`（:16）、`trigger.trigger_ui_searcher`（:17）、`window.autotest_app`（:18）、`project_manager`（:20）、`trigger.trigger_ui_matcher`（:27）、`trigger.trigger_ui_painter`（:28）、`plugin.obj_editor_ui.*`（:30-31）、`trigger.lua-parser.*`（:32-55）、`@appui`（:56-57）、`ini.manager`（:61）、`window.info_window.info_window`（:65）、`window.refer_window.refer_window`（:66）、`trigger.debug.debug_panel`（:67）、`trigger.debug`.new_debugger(V1)（:69）、`ui.load_file_tree_new`（:80）、`window.art_workbench.window_manager.win_app_manager`（:1629）。
- 补丁相关：
  - **总开关：文件头 `if not _G.trigger_editor_on then return end`（:2-4）**。
  - 核心组件 `trigger_editor_component = base.ui.component('trigger_editor', basic)`（:59）。
  - 菜单注册：`EDITOR.event_notify(EVENT.window_title_bar_register, '模块/触发编辑器', create_window, nil, {guide...})`（:1660）；V2 模式下反向 `window_title_bar_unregister`（:1662）。
  - 窗口创建：`window_app.new('TriggerEditor', 100, 100, w, h, false, '触发编辑器')`（:1637）+ `create_ui(trigger_editor_component{...})`（:1638-1642），启动即预创建不显示（:1665）。
  - 大量 EDITOR 事件：`EVENT.open_trigger_debugger`（:1621、:2057）、`EVENT.change_trigger_editor_s_or_c`（:2062）、`EVENT.trigger_editor_init_tree`（:2287）、`EVENT.get_trigger_obj_type_ids`（:2155）、`EVENT.update_trigger_editor_search_box`（:2110）等；快捷键 `shortcutMgr.TRIGGER_EDITOR_*`（:2075-2108）。

## window/backend_script_app.lua
- 用途：后台配置编辑器（数据统计/自定义后台/自定义匹配/排行榜）：左配置文件树 + lite_code 编辑 + 上传。
- 导出：无（执行型脚本，注册菜单与事件）。
- 依赖：`ui.menu_bar`（:5）、`window.window_app`（:6）、`ui.components.window_title`（:7）、`ini.manager`（:8，backend_script_manager :13）、`ui.components.message_window`（:9）、`trigger.ui.message_box_ui`（:10）、`window.set_score_name_window`（:11）、`@common.base.lobby`（:15）、`window.ams_template`（:16）、`sce.map_publisher`（:31）、`cmsg_pack`（:32）。
- 补丁相关：
  - 菜单：`menu_bar.register('设置/自定义匹配配置', ...)`（:900-902）常驻；`'设置/数据统计配置' / '设置/自定义后台配置' / '设置/排行榜配置'` 仅 inner（:882-898）。
  - 窗口按 backend_type 缓存：`window_app.new('BackendScript'..backend_type, 200, 200, 1366, 768, false)`（:768）。
  - 切地图销毁全部窗口：`eventMgr:register_event('on_map_path_changed', ...)`（:909-916）。
  - 数据落盘 `<map>/project/backend/<type>/*.lua`（:525、:574-575）；上传走 `map_publisher.set_backend_script`（:626）。
  - 热更/日志：`lobby.hot_fix_ams`（:440）、`lobby.start_amg_log/stop_ams_log`（:302、:360）。

## window/gui_editor_app.lua
- 用途：界面编辑器（GUIEditor）app 壳：窗口创建 + 快捷键 + 模态选择控件。
- 导出：无（执行型脚本）。
- 依赖：`window.window_app`（:2）、`plugin.gui_editor.ui.main_view`（:3）、`ui.menu_bar`（:4）、`plugin.gui_editor`（:5）、`SCE.GetPluginsManager()/GetUndoRedoManager()`（:8-9）。
- 补丁相关：
  - 菜单在 `EVENT.load_map_done` 里注册（:60-66）`menu_bar.register('模块/界面编辑器', ...)`——**等首次地图加载完成才挂菜单**。
  - 窗口：`window_app.new('GUIEditor', x, y, w, h, false, '界面编辑器')`（:22），UI 用插件的 main_view（:23-27）。
  - 插件兜底加载：`pluginMgr:load_plugin('GUIEditor')`（:52-58）；全局 `guiEditor`（:28、:77）由插件侧提供（本文件未赋值，:53 是 base.next 内局部）。
  - 模态选择：`EVENT.gui_editor_select_ctrl`（:72-85）`set_modal(true)`；`EVENT.force_open_gui_editor`（:68-70）。

## window/ams_template.lua
- 用途：匹配模板代码串库（AI/nVSn/nVSnVSn/OneList/FreeMatch 等，后台配置新建模板用）。
- 导出：模板表（backend_script_app.lua:16、:182-243 引用佐证键名）。
- 依赖：无（纯字符串数据）。
- 补丁相关：无。

## window/autotest_app.lua
- 用途：自动化测试 app：`AutoTestApp = class('AutoTestApp')`（:4），模块注册 `:register(模块, 用例, ...)`（trigger_editor_app.lua:900、:1619 调用佐证）。
- 导出：`AutoTestApp`（menu_bar.lua:2935 `require 'window.autotest_app'` 后 `:show()` 佐证单例用法）。
- 依赖：`window.window_app`（:3）、`window.autotest.autotest_ui`（:5）、`@common.base.argv`（:6）、`window.autotest.utils/const`（:7-8）。
- 补丁相关：窗口 `WindowApp.new('AutoTestApp', 0, 0, 1200, 960, false, '自动测试')`（:20）；菜单「工具/自动测试」仅 inner（menu_bar.lua:2935-2938）。

## window/create_model_app.lua
- 用途：创建模型测试窗口（只有空 ui_template；menu_bar.lua:2838-2845 的注册代码已注释）。
- 导出：未见 return（前 9 行为空模板）。
- 依赖：无。
- 补丁相关：无。

## window/editor_api_window.lua
- 用途：编辑器 API 窗口。**头注释「代码已弃用」（:1）**。
- 导出：组件 editor_api_component 相关（menu_bar.lua:23 仍 require）。
- 依赖：`window.window_app`（:6）、`ui.components.window_title`（:9）、`window.info_window.*`（:10-12）。
- 补丁相关：无。

## window/editor_download_manager.lua
- 用途：资源下载进度 bind（`ResDownloadPregressBind` 继承 DefaultProgressBind，:3）。
- 导出：进度绑定类。
- 依赖：`@common.base.progress`（:2）。
- 补丁相关：无。

## window/editor_local_resource.lua
- 用途：编辑器本地资源记录（已下载列表/预览时间/自动清理，头注释 :1-9）；menu_bar.lua:1003-1004 退出时 `:save()`。
- 导出：含 save 方法的单例表。
- 依赖：未读全（头部为注释）。
- 补丁相关：无。

## window/file_monitor_window.lua
- 用途：项目文件监听（地图目录变更监控，过滤调试压缩包 `.*scene/.-/zip%.7z`，:9-10）。
- 导出：无（执行型，window/init.lua:3 首先 require）。
- 依赖：`@common.base.argv`（:1）、`SCE.GetEventManager()`（:3）、`GetMainFrame():GetMapPath()`（:5）、`ui.components.message_window`（:7）。
- 补丁相关：**与 script 库 unwatch 补丁同领域**——编辑器侧对项目目录的 io 监听入口之一。

## window/guide_window.lua
- 用途：新手引导窗口（component alias/key_frame_state/anim_trans 动画绑定，:4-9）。
- 导出：含 `guide`（trigger_editor_app.lua:81 注释引用佐证）。
- 依赖：`window.window_app`（:1）、`@appui`（:2）、`@common.base.gui.component / control_util`（:4-10）。
- 补丁相关：无。

## window/input_tree.lua
- 用途：appui 风格可输入树组件（`appui_input_tree_content` / `appui_input_tree`，:7-8）。
- 导出：组件（trigger_editor_api_ui_app.lua:6 引用）。
- 依赖：`@appui.theme.themes`、`@appui.components.basic.*`、`@appui.components.tree.declare`（:1-4）、`trigger.lua-parser.utils`（:5）。
- 补丁相关：无。

## window/lod_window.lua
- 用途：LOD 偏移等级设置窗口（菜单「设置/LOD偏移等级」→ `lod_window.show()`，menu_bar.lua:3069-3071）。
- 导出：含 `show`（:8 起 window_instance 懒创建模式）。
- 依赖：`@appui`（:1）、`window.window_app`（:2）、`ui.components.window_title`（:4）。
- 补丁相关：无。

## window/mechanism_editor_app.lua
- 用途：预制功能库 app（`MechanismEditorApp = class(...)`，:2；菜单「模块/预制功能库」:88）。
- 导出：`MechanismEditorApp`（window/init.lua:23 `include 'Window.mechanism_editor_app'` 实例化为 `_G.MECHANISM_MANAGER`）。
- 依赖：`window.window_app`（:1）、`window.mechanism_editor.mechanism_editor_ui`（:4）、`ui.menu_bar`（:5）。
- 补丁相关：菜单直调模式样例。

## window/object_editor_app.lua
- 用途：数据编辑器（物编）app 壳：`show_data_editor(link_data)`（:12）+ 菜单「模块/数据编辑器」（:41）。
- 导出：无（执行型，window/init.lua:4 require）。
- 依赖：`window.window_app`（:2）、`ui.menu_bar`（:3）、`plugin.obj_editor_ui.manager.init`（:10）。
- 补丁相关：真正 UI 在 plugin/obj_editor_ui；本壳只建窗口。

## window/profiler_app.lua
- 用途：Lua Profiler app（`LuaProcessor` 继承 `SCE.ProfilerDataProcessor`，:10；MAX_FRAME=800 :6）。
- 导出：`ProfilerApp`（window/init.lua:22 实例化 `_G.PROFILER_APP`）。
- 依赖：`window.window_app`（:2）、`window.profiler_time_app`（:4）、`profiler.profiler_ui`（:5）、`ui.components.message_window`（:8）。
- 补丁相关：菜单「工具/Profiler」仅 inner（menu_bar.lua:2923-2925）。

## window/profiler_time_app.lua
- 用途：Profiler 时间统计 app。
- 导出：`ProfilerTimeApp`（:6）。
- 依赖：`window.window_app`（:2）、`profiler.profiler_time_ui`（:3）、`ui.components.message_window`（:4）。
- 补丁相关：无。

## window/project_api_window.lua
- 用途：项目 API 版本窗口。**头注释「代码已弃用」（:1）**。
- 导出：组件 project_api_component 相关（menu_bar.lua:22 仍 require）。
- 依赖：`window.window_app`（:5）、`ui.components.window_title`（:7）。
- 补丁相关：无。

## window/project_manager.lua
- 用途：项目管理器 app（启动时 `include 'Window.project_manager'(open_map)`，menu_bar.lua:1265；全局 `PROJECT_MANAGER`，menu_bar.lua:1269）。
- 导出：`ProjectManagerApp`（:2），方法 `:init(open_map) / :show() / :is_active()`（menu_bar.lua:1269-1275、:1614-1624 调用佐证）。
- 依赖：`window.window_app`（:1）、`window.project_manager.project_manager_ui / project_manager_bottom_bar`（:3-4）、`ui.components.window_title`（:5）、`@common.base.lobby`（:7）、`@common.base.argv`（:8）。
- 补丁相关：注意 require 路径大小写写的是 `Window.project_manager`（menu_bar.lua:1265、window/init.lua:23）——Windows 文件系统不敏感所以能工作。

## window/random_terrain_config_app.lua
- 用途：随机地形配置窗口（菜单「设置/随机地形配置」:486）。
- 导出：无（执行型，window/init.lua:6 require）。
- 依赖：`ui.menu_bar`（:10）、`window.window_app`（:11）、`ui.components.window_title`（:12）。
- 补丁相关：无。

## window/random_terrain_template.lua
- 用途：随机地形模板组件（`random_terrain_template`，:4）。
- 导出：组件。
- 依赖：`@appui`（:1）、`SCE.GetPluginsManager()`（:3）、`window.art_workbench.common.res_path_kit`（:5）。
- 补丁相关：无。

## window/record_player.lua
- 用途：操作记录/录像下载播放（`RecordPlayer = class(...)`，:3；含写死的 gfw 文件服务器 URL :9）。
- 导出：`RecordPlayer`。
- 依赖：`@common.base.argv`（:1）、`GetMainFrame()`（:2）、`ui.menu_bar`（:4）。
- 补丁相关：无。

## window/render_capture_upload_app.lua
- 用途：截帧上传 app（`RenderCapture = class(...)`，:10；菜单「工具/截帧上传」仅 doctor argv，menu_bar.lua:2848-2852）。
- 导出：`RenderCapture` 单例（menu_bar.lua:2847 `require ...:show()` 佐证）。
- 依赖：`window.window_app`（:2）、`ui.components.window_title`（:3）、`ui.components.message_window`（:5）、`SCE:GetEProgressBar()`（:8）。
- 补丁相关：无。

## window/resources_manager_app.lua
- 用途：旧资源管理器 app（菜单已注释弃用，menu_bar.lua:2636-2652）。
- 导出：`ResourcesManagerApp`（:2），window/init.lua:18、:28 实例化 `_G.RESOURCES_MANAGER_APP`。
- 依赖：`window.window_app`（:1）、`ui.resource_manager`（:3）。
- 补丁相关：无。

## window/set_score_name_window.lua
- 用途：云变量开通窗口（`set_score_name` 组件，:6；菜单已注释，backend_script_app.lua:904-906）。
- 导出：含 `show`（backend_script_app.lua:11 引用佐证）。
- 依赖：`window.window_app`（:2）、`ui.components.window_title`（:3）、`project_manager`（:9）。
- 补丁相关：无。

## window/texture_merger_app.lua / window/texture_viewer_app.lua
- 用途：图集合并 / 贴图查看 app（`class('TextureMergerApp')` / `class('TextureViewerApp')`，:3；菜单仅 inner，menu_bar.lua:2927-2933）。
- 导出：各自 App 类（window/init.lua:25-26、:39-40 实例化 `_G.TEXTURE_MERGER_APP / _G.TEXTURE_VIEWER_APP`）。
- 依赖：`window.window_app`（:2）、`texture_merger.texture_merger_ui` / `texture_viewer.texture_viewer_ui`（:4）。
- 补丁相关：无。

## window/trigger_editor_api_ui_app.lua
- 用途：触发 API 自定义 UI 查看窗口（`trigger_editor_api_ui` 组件，:4；菜单注册代码已注释 :124）。
- 导出：无显式（执行型，trigger/entry.lua:10 V1 预加载）。
- 依赖：`window.window_app`（:2）、`ui.components.window_title`（:3）、`window.input_tree`（:6）、`trigger.lua-parser.basic_typetree`（:7）、`trigger.ui_rule_new`（:8）、`trigger.lua-parser.utils`（:10）。
- 补丁相关：无。

## window/trigger_validator_editor_app.lua
- 用途：数据编辑器验证器/公式编辑窗口（头注释 :1；`trigger_validator_editor` 组件 :12）。
- 导出：无显式（执行型，trigger/entry.lua:11 V1 预加载）。
- 依赖：`window.window_app`（:2）、`trigger.trigger_editor_ui`（:4）、`trigger.ui.message_box_ui`（:5）、`ui.components.window_title`（:6）、`trigger.lua-parser.parser/utils`（:7-9）、`trigger.trigger_ui_painter`（:10）、`trigger.trigger_ui_matcher`（:11）、`trigger.trigger_manager`（:14）、`trigger.lua-generator`（:16）、`ui.load_file_tree`（:17）。
- 补丁相关：复用 V1 触发编辑器画布（trigger_editor_ui + painter）做验证器编辑；窗口尺寸下限 1920x1080（:27-31）。
