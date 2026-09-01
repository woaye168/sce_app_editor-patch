# xdeditor-160 / ui 目录逐文件研究记录

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/ui/`（86 个 .lua）。
> 全部结论来自真实读取，关键结论标注行号。
> 本目录是 **编辑器主界面层**：菜单栏（menu_bar）、主视图（main_view）、登录（login）、通用组件库（base/ common/ components/ attribute_table/）。

## 核心机制总结

### 菜单栏组件结构（ui/menu_bar.lua，3166 行，本目录最重要文件）

- 本体是 `window_title_bar = base.ui.component('window_title_bar')`（menu_bar.lua:14），`return window_title_bar`（:3166）。**注意：它注册的组件名是 `window_title_bar`，与文件名 menu_bar 不同**。
- **菜单注册三段式**：
  1. `window_title_bar.register(name, callback, key, guide_register, process_type)`（:1100-1115）：`name` 为 `'文件/打开'` 形式的层级路径串；先 `register_command(name, callback)` 存入本地 `callback_map`（:1056-1058），再 `call_cs_function('RegisterItem', {name, shortcut})`（:1114）。
  2. Lua→C#：`call_cs_function` 包装 `MainFrame:SendEvent("EditorMainTitleMenuBar", args)`（:1039-1042），C# 侧模块由 `SCE.Common.create_csharp_module('EditorMainWindow')` 创建（:77）——**菜单本体（原生标题栏）在 C# 侧**，Lua 只持有回调表。
  3. C#→Lua：`eventMgr:register_event('EditorMainTitleMenuBar', function(name) ... call_command(name) end)`（:1066-1069），点击按 name 回查 callback_map 执行。
- **跨模块注册事件**：`EDITOR.event_register(EVENT.window_title_bar_register, function(t, ...) register(...) end)`（:1134-1136）与 `EVENT.window_title_bar_unregister`（:1137-1139）——**其他模块（如 window/*.lua 各 app）通过 `EDITOR.event_notify(EVENT.window_title_bar_register, name, callback, key)` 注册菜单**，这是补丁加菜单的官方通道。
- `process_type` 参数（:1104-1111）配合全局 `ProcessInfo.MainProcess/SubProcess` 过滤主/子进程注册。
- 注销：`unregister(name, hide)`（:1126-1132）。
- 本文件还承担：游戏调试全流程（`_debug_save_as`，:348-752；任务管线 `DebugGame`，:884-936）、地图保存事件 `EVENT.save_map`（:957-975）、退出编辑器 `exit_editor`（:998-1035，其中 `_G.WINDOW_APP_MANAGER:close_all()` :1005）、命令行发布分支 `_G.generate_cmd`（:1228-1260）、打开项目 `include 'Window.project_manager'(open_map)`（:1265，注意大小写）、登录按钮初始化（:3079-3147）、大量 inner argv 专属菜单。
- 打开的 app 全局：`_G.RESOURCE_STORE_APP`（:2656）、`_G.ART_WORKBENCH_APP`（:2672）、`PROFILER_APP / TEXTURE_VIEWER_APP / TEXTURE_MERGER_APP`（:2924-2933）、`PROJECT_MANAGER`（:1269）。
- **模拟多人调试段**（2026-09-01 实机实证，全文 doc/research/multi-player-debug.md）：菜单「调试/模拟多人调试」（:1806-1819）只激活 C# 对话框 MutiDebugWindow；对话框确认 = `SendEvent('CS_muti_debug', json)` → :1752 处理器（数编三键 Game.player_setting/opened_slots/user_ids 校验 + 补 UserId/SlotID + 填 debug_user_info）→ `debug_save_as{muti_debug_info=t, use_muti_debug=true}`（:1803）。槽位 = `GamePlayInEditor1..4`（:1799，单人 = 无序号）；逐槽 register_plugin_ui + show_game_in_editor（:675-693，Delay>0 走 base.wait 错峰）。「使用上次配置」两菜单被 `argv.has('inner')` 门控（:2106-2123）且 last_multi_debug_info 纯内存（:873）。**编程直调路径（已 spike 验证）**：`require('ui.menu_bar').debug_save_as{...}`（导出 :3163-3164），动态代码里须用 `package.loaded['@xdeditor/ui/menu_bar']` 绝对键（load 上下文 require 会落到 @common 包）。tab 三事件：关闭毁局计数（:644-653）/ 暂停 disconnect/reconnect（:656-664）/ 切换仅 set_game_ui_focus（:668-671）。

### 主视图（ui/main_view.lua）

- `main_view = base.ui.panel{...}`（:18-129）：`menu_bar{}`（:27）+ dock_area（场景列表 `scene_list` / 属性窗口 / 场景视图 `resource_view`，:44-107）+ info_panel_parent + foot。
- 导出 `{ ui = main_view, init = init, menu_bar = menu_bar }`（:205-209）——**menu_bar 组件经 main_view 再导出**，是拿到 window_title_bar 单例的正路（`require 'ui.main_view'.menu_bar`）。
- init(bind)（:188-203）初始化 shortcut/confirm/navi/scene_view/属性窗口/create_panel；foot 仅 inner 模式显示（:197-201）。
- 样式/分辨率事件：`EVENT.style_change`（:166）、`EVENT.resolution_change`（:169）。

### 编辑器 UI 引导（ui/init.lua）

- `include 'ui.window_ui'`（:1，打 base.ui 补丁）→ 装配全局 `sce.ui = {tree/input/select/button/menu/border/.../color_input}`（:4-31）→ `include 'ui.main_view'` → `base.ui.create(main_view.ui)` + `main_view.init(bind)`（:33-36）。
- **加载时机：本文件即编辑器主 UI 的构建入口**，执行后 sce.ui 全局组件库可用。

### window_title 组件（ui/components/window_title.lua）

- `base.ui.component('window_title', appui.ui.basic)`（:4），`return window_title`（:351）。所有 window/*.lua app 的窗口标题栏（拖动/最小化/最大化/关闭/帮助链接）。
- props（:8-83）：`hide`（关闭转隐藏，:9-12）、`enable_min/enable_max/enable_drag`、`close_on_click`、`on_window_hide`（:32-34，hide 关闭时给外部回调）、`highlight_tip`（:36-43）、`title`、`help_link_tag`（:72-77，经 `EDITOR.utils.get_doc_url` 取帮助链接 :329-337）、`menu`（TODO :79）。
- 关键：经 `base.ui.get_ui_window(self.ui)` 反向拿宿主 FWindow（`owner_fwindow`，:169-175）；监听引擎事件 `'close_window'` 同步标题栏关闭（:269-274），`on_remove` 反注册（:281-285）。

### 登录（ui/login.lua）

- `LoginWindow = base.ui.component('login_window')`（:3），导出 `start`（:670）。头部注释「有些修改是这段代码迁移到xdeditor_startup时修改的」（:1）。
- 扫码登录（TapTap token，`set_token` :27-45 → `account.set_token/set_access_token/save`）；UI 模板 z_index 9999999 全屏遮罩（:47-51）。
- 依赖全走 `@base.base.*`（account/lobby/argv/ip，:4-10，@ 跨库 client_base）。

---

## 根级文件（其余简录）

## ui/window_ui.lua
- 用途：给 `base.ui` 打补丁：`create_ui_root/destroy_ui_root`（:3-15），把引擎 window 的 ui_root 纳入 base.ui.map 管理。
- 导出：无（执行型，直接改 base.ui）。
- 依赖：`ImportSCEContext()`（:1）。
- 补丁相关：**被 ui/init.lua:1 最先 include**——base.ui 扩展点范本。

## ui/window_view.lua
- 用途：`WindowView = class('WindowView', include 'ui.base_view')`（:1），窗口视图基类封装。
- 导出：`WindowView`（:12）。
- 依赖：`ui.base_view`。
- 补丁相关：无。

## ui/base_view.lua
- 用途：`BaseView = class('BaseView')`（:1），视图基类（ui/bind 持有 + 事件管理器）。
- 导出：`BaseView`（:70）。
- 依赖：`ImportSCEContext().GetEventManager()`（:3-4）。
- 补丁相关：无。

## ui/utils.lua
- 用途：触发器保存侧工具库：`get_origin_librarys / get_origin_lua_file`（entry.lua:21-22 调用佐证）等 lua+ → 原生 lua 落地逻辑（739 行）。
- 导出：表（:739）。
- 依赖：`trigger.trigger_ui_painter`（:1）、`trigger.lua-generator`（:3）、`trigger.trigger_manager`（:4、9）、`lua-parser.utils`（:5-7）、`basic_typetree`（:8）、`project_manager`（:10）。
- 补丁相关：**V1 触发保存链的落脚点**（`EVENT.save_map_progress_trigger_editor` → get_origin_lua_file）。

## ui/test.lua
- 用途：UI 控件手动测试脚本。
- 导出：无（执行型）。
- 依赖：`@common.base`、`@common.class`、`global/utils/config/console`（:1-6）。
- 补丁相关：无。

## ui/splash.lua
- 用途：启动闪屏面板（z_index -10 垫底）。
- 导出：`base.ui.panel{...}`（:3）。
- 依赖：`include 'config.ui.style'`（:1）。
- 补丁相关：无。

## ui/shortcut_ui.lua
- 用途：快捷键提示 UI + 快捷键配置（`get_keys`，menu_bar.lua:27 引用）；配置存 `GetMainFrame():GetUserPath() .. "shortcut.json"`（:5）。
- 导出：表（:662），含 `init / get_keys`。
- 依赖：`ImportSCEContext().GetPluginsManager()`（:2）、`@appui`（:3）、`@common.json`（:4）。
- 补丁相关：无。

## ui/scene_view.lua
- 用途：场景视图容器面板（含边框、场景提示）。
- 导出：`{ ui = scene_view, init = init }`（:66）。
- 依赖：`@appui`（:1）、`ui.common.scene_tips`（:2）。
- 补丁相关：无。

## ui/res_tree_ctrller.lua
- 用途：资源树控制器（注释 :1「合并 ToolTreeController 和 resource_manager」），新建/移动/复制/删除资源操作 + 撤销。
- 导出：`resTreeCtrller` 类（:3、:715）。
- 依赖：`SCE.GetUndoRedoManager()`（:8）、`plugin.tile_editor.filesystem`（:5）、`@common.base.argv`（:4）。
- 补丁相关：无。

## ui/res_config.lua
- 用途：资源目录配置（963 行），资源库导入/目录映射数据与逻辑。
- 导出：`return`（:963，多值）。
- 依赖：`plugin.model_editor.windows.*`（:2-4）、`ui.res_tree_ctrller`（:5）、`ui.res_lib_manager.res_lib_move`（:8）、`plugin.particle_editor.utils`（:9）。
- 补丁相关：无。

## ui/resource_tree_tool_ui.lua
- 用途：资源树右键菜单/弹出框（注释 :1），`show_menu`。
- 导出：表（:460）。
- 依赖：`SCE.GetPluginsManager()/GetSceneManager()`（:3-4）、`ui.common.menu_tree`（:6）、`plugin.tile_editor.terrain_select_view`（:7）、`trigger.lua-parser.utils`（:10）。
- 补丁相关：无。

## ui/resource_tree.lua
- 用途：旧版资源树 UI（含 `log.error('1')` 调试残留 :8）。
- 导出：表（:582）。
- 依赖：`trigger.trigger_manager`（:2）、`ui.common.title_panel`（:6）、`ui.components.tree_with_search`（:7）、`ini.object_tree.node_type`（:9-10）。
- 补丁相关：无。

## ui/resource_manager.lua
- 用途：旧版资源管理器窗口（769 行）。
- 导出：表（:769）。
- 依赖：`config.ui.style`（:1）、`@common.base.p_ui.datagrid/grid`（:2-3）、`@common.update`（:6）、`ui.res_config`（:7）、`plugin.tile_editor.filesystem`（:8）。
- 补丁相关：无。

## ui/resolution_change_window.lua
- 用途：分辨率切换确认窗口（250 宽面板）。
- 导出：表（:86）。
- 依赖：`@appui`、`appui.theme`（:1-2）。
- 补丁相关：无。

## ui/operation_recorder_view.lua
- 用途：undo/redo 操作记录可视化（注释 :1）。
- 导出：无显式 return（类定义文件，`OperationView` :6）。
- 依赖：`@appui`（:2）、`SCE.GetUndoRedoManager()`（:4）。
- 补丁相关：被 main_view.lua:16 直接 require（仅加载即生效）。

## ui/operation_menu.lua
- 用途：场景操作菜单（撤销/重做/游戏内调试标签页操作）。
- 导出：无显式 return（执行型）。
- 依赖：`SCE.GetUndoRedoManager()/GetPluginsManager()/GetSceneManager()`（:3-5）、`ui.menu_bar`（:7）、slot_ids GamePlayInEditor 系列（:8-10）。
- 补丁相关：无。

## ui/navi.lua
- 用途：导航条面板（当前 main_view 中已注释停用，main_view.lua:28-31）。
- 导出：`{ ui = navi, init = ... }`（:19）。
- 依赖：无。
- 补丁相关：无。

## ui/lobby_debug_button_view.lua
- 用途：大厅调试按钮视图（EmmyLua 生成头）。
- 导出：`show_lobby_debug_button_view` 函数（:89）。
- 依赖：`config.localizatioin.*`（:7-8）、`@common.base.util`（:9）、`@appui`（:10）。
- 补丁相关：menu_bar.lua:2415 调用。

## ui/load_file_tree_new.lua / ui/load_file_tree.lua
- 用途：代码/资源树加载与右键菜单接转（new 版带 s_or_c 双端参数）。
- 导出：`load_file_tree`（:102 / :114）。
- 依赖：`ini.manager`（:1-3）、`ui.resource_tree_tool_ui`（:4）、`trigger.trigger_manager`.read_sign（:5-6）。
- 补丁相关：无。

## ui/gameplay_in_editor_view.lua
- 用途：编辑器内运行游戏（PIE）的插件 UI 视图工厂。
- 导出：`function(id, debug_user_info)`（:840）——menu_bar.lua:676/718 `pluginMgr:register_plugin_ui(slot_id, gameplay_in_editor_view(...))` 佐证。
- 依赖：`@appui`（:1）、`SCE.GetPluginsManager()/GetSceneManager()`（:4-5）、`plugin.tile_editor.ui_resolution_content`（:6）、`ui.components.message_window`（:7）、`plugin.obj_editor_v2.const`（:9）。
- 补丁相关：**多人调试实证（2026-09-01）**：视口控件 name = 传入 id（:105-117），base.ui.map 键 = 窗口根 `GamePlayInEditor<N>` + 视口控件 `ui-<n>-GamePlayInEditor<N>`；tab 标题 `玩家 N 视图`/图标着色由 debug_user_info 驱动（:123-131）；多视口 get_screen_rect 同值（同 dock 叠放），无可见性 getter，分客户端截图须先 `_G.ui.switch_page(slot)` + `sceneMgr:set_game_ui_focus(slot)` 切前台（**切焦后 ~45ms 即合成完成**，像素采样实测）；暂停 = disconnect_game_in_editor（VM 停止应答 dbg 广播），切 tab 不自动 reconnect，**暂停态无官方查询**（sceneMgr 全方法仅 hide/show/disconnect/reconnect/set_game_ui_focus/is_scene_focus）。**离场重进坑（2026-09-02 两轮实测）**：hide_game_in_editor 即销毁 C++ 侧槽位会话（服务端真离场），再 show_game_in_editor 会挤掉在局玩家并只重建单客户端会话——局内加人/重进不可行。

## ui/foot.lua
- 用途：底部状态栏（仅 inner argv 显示，main_view.lua:197-201）。
- 导出：表（:94）。
- 依赖：`config.localizatioin.ui`（:1）、`config.ui.style`（:2）、`@common.base.argv`（:3）。
- 实证（2026-09-02）：左下角 `[Connected/Disconnect] <本机IP>:6251` 指示器 = **手机真机调试服务**状态（菜单「调试/手机调试」→ `DebugManager:phone_debug()`，menu_bar.lua:2399-2412；6251 为写死端口）。状态源 = init 时 `DebugManager:server_is_active()` + 引擎事件 `connect_state_changed` 切换（foot.lua:66-91，仅 inner 版点亮 foot_info_show）。地址部分 = `common.get_local_ip()`——给用户看「手机应连的地址」；显示 0.0.0.0 = 未取到有效本机 IP（多网卡/VPN/无活跃网卡兜底）。**与调试 host（游戏服务端）无关**：6251 是编辑器=服务端、手机=客户端连入的真机调试通道。
- 补丁相关：无。

## ui/editor_world_ui.lua
- 用途：世界地图调试输入弹窗（show_editor_world_id/show_editor_mode/show_editor_world_type）。
- 导出：表（:283）。
- 依赖：`@appui`（:1-2）。
- 补丁相关：menu_bar.lua:28 引入 `editor_world_id`。

## ui/download_world_map.lua
- 用途：世界地图下载（extra_maps，:7）。
- 导出：`download_world_map`（:112），方法 `:download_world(world_id, path)`（menu_bar.lua:320 佐证）。
- 依赖：`@common.update.download_manager`（:1）、`@common.update.core.local_version`（:3）、`@common.base.co`（:4）、`@base.base.util`（:5，@ 跨库）。
- 补丁相关：无。

## ui/confirm_ui.lua
- 用途：场景操作确认条 UI。
- 导出：`{ ui, init }`（:369）。
- 依赖：`@appui`（:1）、`SCE.GetPluginsManager()`（:3）、`plugin.tile_editor.select_list_view.scene_list_manager`（:5）。
- 补丁相关：无。

## ui/collide_operation_view.lua / ui/collide_operation_ui.lua
- 用途：碰撞（体型）操作视图 / 其 UI 模板（make_human 插件配套）。
- 导出：表（:195）/ `collide_operation_ui`（:377）。
- 依赖：`plugin.make_human_plugin.common/event`（view :1、:7）、`config.ui.style`（ui :3）。
- 补丁相关：无。

## ui/attribute_view.lua
- 用途：属性视图窗口（`appui.ui.window`，:6）。
- 导出：`{ ui, init }`（:142）。
- 依赖：`@appui`（:1）、`ui.components.table_form`（:2）、`SCE.GetPluginsManager()`（:4）。
- 补丁相关：无。

## ui/view/（3 个，选项弹窗）
- **save_option_view.lua**：保存选项弹窗，`SaveOptEnum = {Cancel=0, Discard=1, Save=2}`（:6-10），导出含 `open_window(co)`（menu_bar.lua:988 佐证协程式用法）。
- **upload_option_view.lua**：发布选项弹窗，`UploadOptEnum = {Cancel=0, Upload=1, ...}`（:6-8）。
- **autosave_config_view.lua**：自动备份设置窗口（已停用，menu_bar.lua:1608-1612 注释）。
- 三者均依赖 `config.localizatioin.localization` + `@appui` + SCE MainWindow。

## ui/res_lib_manager/（5 个，资源库管理器组件）
- **res_lib_title_bar.lua**：`base.ui.component('res_lib_title_bar')`（:6），导出 :102。
- **res_lib_contents_view.lua**：`base.ui.component('res_lib_contents_view')`（:6），导出 :157。
- **res_lib_bottom_bar.lua**：`base.ui.component('res_lib_bottom_bar')`（:6），导出 :90。
- **res_lib_move.lua**：本地资源整理（deco 目录，注释 :1-2），导出表 :455。
- **res_move_tool.lua**：资源操作工具（新建/移动/复制/删除，注释 :1-4，1605 行），依赖 `window.art_workbench.common.res_path_kit`、`modeling_editor.common.save_window`（:6-7）。

## ui/resource_explorer/（3 个）
- **re_tree_panel.lua**：带搜索栏的资源树 panel（:7-8 注释），导出 :127。
- **re_tree.lua**：`re_tree_content` 组件（:4），导出 :150。
- **re_input.lua**：资源搜索输入框组件（继承 ui.base.input，:6），导出 :42。

## ui/components/（12 个）
- **window_title.lua**：详写见上。
- **tree_with_search.lua / tree_with_search_new.lua**：带搜索栏的树组件（:5），导出 :105/:106。
- **table_form.lua**：多层结构 table 编辑表单（头注释 :1-8），导出 :223。
- **snapshot.lua**：快照上传组件（http_requests.goods + art_workbench_settings，:7-8），导出 `snapshot_component` :266。
- **scene_view_bar.lua**：资源管理器场景视图上方工具条（:1，ModelEditorPlugin :4），导出 :86。
- **play_pause_component.lua**：播放/暂停组件（:5），导出 :196。
- **message_window.lua**：确认/取消/关闭消息弹窗函数集，`OptEnum = {Close=1, Cancel=2, Confirm=3}`（:4-8），导出表 :158，含 `message_window(callback, texts, msg, title)`（menu_bar.lua:1013 调用佐证）。
- **color_input.lua**：`color_input` 组件（:3），导出 :305。
- **button_bar.lua**：`button_bar` 组件（:6），导出 :280。
- **form/form.lua**：表单组件（聚合 slider/vector/array/resource_select，:3-8），导出 :197。
- **form/resource_select.lua**：`resource_select` 组件（:3），导出 :66。

## ui/common/（15 个，通用组件，全部经 sce.ui 或直接 include 使用）
- **window.lua**：`window` 组件（:3），dock_target 等 props，导出 :127。
- **toggle_panel.lua**：折叠面板（:4），导出 :147。
- **title_panel.lua**：`title_panel`（继承 tabs_panel，:4），导出 :31。
- **tip_panel.lua**：气泡提示面板（对齐逻辑 align_map :5-6），导出 :308。
- **tips.lua**：`tips = class("tips")`（:2）提示管理类，导出 :91。
- **tabs_panel.lua**：`tabs_panel`（继承 focus 组件，:3），导出 :171。
- **scene_tips.lua**：场景提示内容（EmmyLua 头），导出 :201。
- **message_window.lua**：`message_window` 组件版（:3，与 components/message_window 函数版并存），导出 :465。
- **menu_tree.lua**：树形菜单组件（`menu_width` :4 起），导出 `menu_tree` :241。
- **localization_label.lua**：多语言 label（:1，current_language :3），导出 :57。
- **guide.lua**：新手引导注册（`guide_register`，menu_bar.lua:1735 等菜单 guide 参数使用），导出表 :600。
- **color_setter.lua**：`color_setter` 组件（:6），导出 :288。
- **color_packer.lua**：`color_packer` 取色器组件（:6），导出 :621。
- **border.lua**：`border` 组件（继承 focus，:3），导出 :94。

## ui/base/（5 个，基础控件）
- **tree.lua**：`tree_content`（:3）+ `tree` 组件（导出 :631），头注释「看tree的属性请搜索 tree:define()」。
- **select.lua**：`select` 下拉组件（继承 focus，:1），导出 :354。
- **menu.lua**：`menu` 菜单组件（:2，注释「未完成」），导出 :226。
- **input.lua**：`input` 输入框（TYPE 定义 :4-6），导出 :193。
- **button.lua**：`button` 按钮（:2），导出 :131。

## ui/attribute_table/（11 个，属性表单项组件，均 base.ui.component）
- **attribute_table.lua**：`attribute_table`（:7，值变即写回 table 字段，注释 :1），导出 :214。
- **simple_form.lua**：`simple_form`（:15，只发 on_change 不改值，注释 :1），导出 :332。
- **array.lua**：`attribute_table_array_row`（:4）+ `attribute_table_array`（:6），导出 :244。
- **slider.lua**：`slider`（:3），导出 :211。
- **vector.lua / vector2.lua**：`vector`（:4）/ `vector2`（:4），导出 :111/:110。
- **ratio_buttons.lua**：`ratio_buttons`（:3），导出 :93。
- **label.lua**：`attribute_table_label`（:2），导出 :35。
- **input_select.lua**：`input_select`（继承 focus，:4），导出 :84。
- **drag_input.lua**：`drag_inner_input`（:15）+ `drag_input`（:34），导出 :300。
- **resource_select.lua**：`resource_select`（:3），导出 :84。
