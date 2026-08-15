# xdeditor-160 / 批次A 逐文件研究记录（根级 main.lua、io_modifier.lua + config/ + console/ + examples/ + exception/ + global/ + guide/）

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/`（明文镜像，部分注释 GBK 乱码已忽略）。
> 本批共 41 个 .lua：根级 2 + config/ 22 + console/ 3 + examples/ 2 + exception/ 3 + global/ 2 + guide/ 7。
> 全部结论来自真实读取，关键结论标注 相对路径:行号。

## main.lua 加载流程主线（重点）

`main.lua` 是 xdeditor 库入口（C++ 拉起 Lua state 后执行），顶层顺序执行，分支全部用 `argv.has(...)` 判断命令行参数：

1. **公共前奏**（main.lua:1-8）：`pcall(collectgarbage, 'generational')`（:1）→ `app.set_version_key_value('open_package_holding', 'value')`（:3-5）→ `require '@common.base'`（:6，跨库到 script 的 common）→ `include '@common.base.argv'`（:7）→ `require '@common.base.util'`（:8）。
2. **unit_test 分支**（:11-18）：`common.has_arg("unit_test")` 时跑 `@common.base.example.main` 并 `return`，**之后所有分支都不走**。
3. **scene_test 分支**（:20-29）：`include 'global'`（:21）→ `include 'utils'`（:23）→ `include 'config'`（:24）→ `require 'console'`（:25）→ `base.ui.auto_scale.disable()`（:26）→ `require 'scene_test_enter_point.main'`（:27）→ return。注释 :22「这些会注册一些全局变量, 注意变量会依赖」——**global/utils/config 三件套是全局注册器，顺序有依赖**。
4. **sub_process 分支**（:31-44）：先 `io.set_package_io_mode(0)`（:32-35），再同样三件套 + console，最后 `require 'sub_process_enter_point.main'`（:42）→ return。
5. **generate_cmd 标记**（:46-66）：`upload_map` / `upload_lib` / `upload_lib_abs` / `generate_map` 参数设置 `_G.generate_cmd`；`trigger_editor_v2_developer` 设 `_G.trigger_editor_v2_developer = true`（:56-58）；有 generate_cmd 时挂 2 分钟超时定时器 `_G.generate_cmd_timer`，超时 `os.exit(1)`（:60-66）。
6. **清 shader 缓存**（:68-72）：无 `no_clear_shaders` 参数时 `common.clear_shaders()`（:70，C++ 全局）。
7. **主流程公共加载**（:74-127）：`require '@common.base.lobby'`（:74）、`include` lobby/account/co/check_log/platform（:77-81）、`require '@common.base.ip'`（:82）、**`local SCE = ImportSCEContext()`**（:83，C++ 注入的编辑器上下文，之后 `SCE.GetUndoRedoManager()/GetEventManager()/GetPluginsManager()` :84-86）→ `account.init()`（:111，加载保存的账号/guest_id）→ **`include '@common.class'`（:117）→ `include 'global'`（:118）→ `include 'utils'`（:120）→ `include 'config'`（:121）→ `require 'console'`（:122）→ `include 'exception'`（:123）→ `include 'sub_process_enter_point.init_process_info'`（:124）**——这是主流程的全局初始化五连，global 最先（注册 EDITOR/EVENT），utils 次之（挂 EDITOR.utils/event_register/event_notify），console 注册快捷键与 trigger_editor_on 开关，exception 注册断网弹窗，init_process_info 注册 `_G.ProcessInfo`。
8. **登录回调主线**：`continue_launch_editor()`（:710-792）——`generate_and_debug_map` 时 `include 'ui.login'` 登录后 `require 'map_starter'`（:711-718）；`local_test` 且不允许更新时直接 `show_editor_main_ui()`（:726）；否则 `try_update_and_launch_editor()`（:732-788）：`check_log.start()`（:735）→ `shortcut.load_shortcut_configs()`（:737）→ `auto_remove_preview_resource()`（:739）→ `test_resource`/`doctor` 参数处理（:741-771）→ **非 local_test 时 `include 'ui.login'` 并 `login(function() ... show_editor_main_ui() end)`（:773-781）**，登录回调内依次 `update_modules_to_update_in_xdeditor()`、`EDITOR.utils.update_http_ip()`、`update_editor_resource_dict()`、`show_editor_main_ui()`。
9. **`show_editor_main_ui()`**（:444-657）——编辑器主界面真正拉起处：
   - 设分辨率/窗口位置（:447-462），`_G.DURING_SPLASH_WINDOW = false`（:464），日志 `X.D.Editor start..`（:469）。
   - **`require 'io_modifier'`（:471）→ `include 'ui'`（:473）→ `include 'window'`（:475）→ `include 'plugin'`（:477）**——四大 UI/插件目录在此才加载，即 io_modifier 的 io hook 先于一切 UI 代码。
   - `require "trigger.trigger_manager"`（:479）、`require "project_manager"`（:480）。
   - 注册 `'reload'` 事件：清 trigger 变更 + undo/redo + `app.reload()`（:482-495）。
   - `init_map_path_serarcher_event()`（:499，函数定义 :320-354）：注册 `EVENT.load_map`（加载 project 文件 + `SCE.Common.set_package_to_path_searcher` 注册项目名/项目 id 两个 require 前缀 :337-341）与 `EVENT.unload_map`（注销 :344-353）。
   - `init_tips(true)` 协程读积分提示文本后 `load_map()`（:586-626）；`load_map()`（:543-584）内 `EDITOR.update_map_libs(map_path)`（:546）→ `EDITOR.load_map(map_path, true, nil, save_map_impl回调)`（:563）。
   - 收尾：`require 'window.art_workbench'`（:631）、autotest_app/record_player 的 argv 任务（:635-644）、`GetMainFrame():SetSCEEditorState(97)`（:654-656）。
10. **登录后主流程尾部**（:794-959）：`SCE.StartListenInputEvent()`（:795）；shortcutMgr 注册 UNDO/REDO/NEW/OPEN/SAVE/RELOAD_SHADERS 等全局快捷键（:806-901），其中 NEW/OPEN/SAVE 转发到 `sce.ui.main_view.menu_bar.callback_map['文件/新建'|'文件/打开'|'文件/保存']`（:800-803、:867-890）——**menu_bar 的 callback_map 是快捷键到菜单的桥**；`base.game:broadcast('send_logs', ...)`（:905-908）；`_G.upload_log = require '@common.base.upload_log'`（:912）；`argv.has('show_examples')` 时 `require 'examples'`（:914-916）；`debug.dump_traceback` 内存泄漏定时 dump（:919-950）；`tiledSceneMgr:create_tiled_scene(EFFECT_TILED_SCENE_NAME, 128, 128)`（:952-956）；`argv.add('kcp_stream', '1')`（:958）。
11. **返回值**（:961-965）：`return { continue_launch_editor = continue_launch_editor, argv_has_scene_test = argv_has_scene_test, argv_has_sub_process = argv_has_sub_process }`——注意 **argv_has_scene_test / argv_has_sub_process 在本文件从未定义**，是未定义全局（nil），疑似历史遗留。`continue_launch_editor` 由外部（StartUp 包 @xdeditor_startup）回调调用，注释 :731「StartUp里已经把这个函数的前半段执行完了」。

**补丁含义**：
- 主流程入口插槽应插在 main.lua 顶层 `return`（:961）之前；三件套 include（:118-121）之后所有全局（EDITOR/EVENT/EDITOR.utils/EDITOR.event_register）已就绪。
- 想 hook 主界面加载完成，可靠挂点是 `EVENT.load_map_done`（utils/event.lua:109 发出）或 menu_bar 注册事件 `EVENT.window_title_bar_register`（utils/event.lua:314 发出）。
- `EDITOR.event_register('reload', ...)`（main.lua:482）说明 **'reload' 是字符串事件名**（不走 EVENT 常量表），由 console 或子进程 F1 触发（sub_process_enter_point/main.lua:34-38）。

## global/global.lua 事件常量清单（重点）

- `EDITOR = { test = true }`（global/global.lua:2-4）——**EDITOR 根表在此创建**，后续 utils/event.lua 等往里挂函数。`EDITOR.test = true` 是常驻开关（utils/utils.lua:1190、utils/math.lua:46 用它跑自检）。
- `EVENT`（global/global.lua:7-211）：纯字符串常量表（key 与 value 基本同名，少数不同已在下面标注），按注释分组：
  - 通用：style_change、resolution_change、debug_game_resolution_change、language_change（:8-11）
  - 地图生命周期：**add_lib_path（:13，load_map 前把 libs.json 引用库加进 pathsearcher）、load_map（:14）、load_map_done（:15）**、update_project_settings（:16）、finish_load_scene = `'resource_tree_load_scene_finish'`（:17）、**unload_map = `'unload_map_and_hide_tile_editor'`（:18）**、save_map（:19）、save_map_progress_obj_editor / _tile_editor / _gui_editor / _mechanism_editor / _trigger_editor_v2 / _trigger_editor / _obj_ref（:20-27，存图管线各编辑器分阶段事件）、force_open_gui_editor（:24）、resource_manager_load_res（:29）、viewport_input_points_coordinate_update（:30）、save_points_name（:31）、reload_obj_data = `'reload_object_editor_data'`（:32）
  - 模型：import_model、update_ctrl_text、import_animation（:34-36）、create_model、finished_create_model、selected_bone_changed、create_attribute_unit、on_anim_changed、on_preview_mesh_node_refresh、on_preview_lock_status_refresh（:39-45）
  - 树/选择：unit_tree_on_change、trigger_tree_on_change、scene_tree_on_change、ini_attribute_view_on_change、show_unit_by_data、reimport_model（:47-52）
  - 快捷键转发：play_animation、control_and_p_press、control_and_s_press、control_and_f_press（:54-57，由 console/keyboard.lua 发）
  - 新手引导：guide_next_page、guide_previous_page、guide_quit、guide_finish、guide_jump_page（:60-64）
  - 地形/地编：tile_editor_enter_free_state、tile_editor_viewport_show、tile_editor_viewport_hide、update_map_size_by_random_terrain_settings（:67-70）、set_select_mode、select_item_list、on_select_mode_activate、on_unit_model_path_change、on_resource_tree_remove_unit（:72-76）、on_atmosphere_set、on_atmosphere_add（:79-80）、create_panel_operation（:83）、check_unit_in_imgui_tree（:86）、terrain_on_set_camera_view_mode（:89）、autosave_start、autosave_end（:92-93）、change_show_grid_collision（:96）、change_show_fog（:99）、change_show_indicator（:102）
  - 触发编辑器 v1：trigger_editor_pre_require（:105，打开地图时决定加载 v1/v2）、read_lua_file、update_require_enum、generate_ui_rule、ungenerate_ui_rule（:106-109）、trigger_variable_on_change、rename_trigger_file、update_node_change、trigger_editor_init_tree、trigger_editor_clear_obj_event（:111-115）、**download_map_ref_libs、download_map_ref_resources、download_map_resources_from_ref_package（:116-118，utils/map_download_refs.lua 监听）**、trigger_editor_reload、trigger_validator_editor_reload（:119-120）、item_editor_loaded、trigger_editor_ui_message_box（:121-122）、tile_editor_item_select_button、close_tile_editor_item_select_button、tile_editor_item_set_name、tile_editor_item_remove（:123-126）、update_trigger_editor_search_box、on_open_trigger_file、on_open_trigger_file_by_location、goto_tile_editor_item、tile_editor_scene_crud、enum_define、trigger_left_tree_focus、set_define_path、set_variable_type（:127-135）、add_trigger_info、remove_trigger_info、get_trigger_obj_type_ids、get_trigger_obj_type_ids_v2、get_all_references_in_trigger_editor、open_trigger_editor（:137-142）
  - 触发调试器：debug_trigger_status、trigger_debugger_tree_apply、trigger_debugger_save_watch、trigger_debugger_watch_list_operation、trigger_debugger_variable_list_operation、trigger_debugger_change_breakpoint、set_trigger_stack、set_trigger_current_stack、set_trigger_variable_list、set_trigger_breakpoint_list、update_watched_variable、trigger_debugger_init、open_trigger_debugger、update_debugger_trigger_global_variable（:145-158）
  - 其他：backend_script_on_change（:161）、data_editor_operation（:164）、add_info_list、remove_info_list、clear_info_list、set_info_list_panel、show_info_list、update_info_title（:167-172）、set_refer_list、set_trigger_refer_list（:176-177）、update_indirect_reference（:180）、obj_init（:183）、localization_on_loadmap = `'localization_manager_on_loadmap'`（:186）、localization_on_savemap、localization_on_language_change、localization_on_language_change_editor（:187-189）、**window_title_bar_register、window_title_bar_unregister（:191-192，标题栏菜单注册/注销，editor-patch menu_bgd 模块的同类挂点）**、pre_init_module_id、trigger_editor_v2_jump、trigger_editor_v2_validator_jump_line、get_trigger_editor_v2_draw_node、trigger_editor_v2_predict、open_formula_editor_window、update_trigger_v2_libs、change_trigger_editor_s_or_c、update_trigger_v2_obj_time_stamp、trigger_v2_backward（:195-204，触发器 v2）、gui_editor_select_ctrl（:207）、pie_will_launch（:210）
- `CPP_EVENT = { on_update_progress_of_save_change_cloth = 'on_update_progress_of_save_change_cloth' }`（:212-214）——**CPP_EVENT 全表只有这一个事件**。
- 其他全局：`RENDER_PATH`（:217-228，10 个渲染路径配置 default/bloom/bloom_map/cipr/bloom_ssao/bloom_vdm_low/bloom_vdm/snapshot/tone_map/deferred_map）、`EFFECT_TILED_SCENE_NAME = "@tiled_scene_effect_preview"`（:230）、`MAP_TASK_PRIORITY`（:232-245，CREATE_PROJECT/LOAD_PROJECT 任务优先级枚举）。

## utils/event.lua 事件机制（重点，文件在批次B记录，此处引用结论）

`EDITOR.event_register` / `EDITOR.event_notify` 定义在 utils/event.lua:10-18：本质是 **`base.event_register(editor_events, name, callback)` / `base.event_dispatch(editor_events, name, ...)`**（base 为 script 库 common 的 C++/Lua 混合基础库），`editor_events` 是本文件局部表（utils/event.lua:4）。回调第一个参数是 trigger/sender。注册返回 connection；notify 同步派发并返回最后一个回调的返回值（save 管线用 promise 参数回传结果，utils/event.lua:439-451）。同文件还把 load_map/unload_map/save_map/upload_map 等实现挂到 EDITOR（:804-815）。

## 逐文件记录

## main.lua
- 用途：xdeditor 库入口与加载主线（argv 分支 → 全局初始化 → 登录 → show_editor_main_ui）。
- 导出：`return { continue_launch_editor, argv_has_scene_test, argv_has_sub_process }`（main.lua:961-965，后两个是未定义全局 nil）。
- 依赖：`@common.base`（:6）、`@common.base.argv`（:7）、`@common.base.util`（:8）、`@common.base.lobby/account/co/check_log/platform/ip`（:74-82，@ 跨库）、`@common.class`（:117）、`global/utils/config/console/exception`（:118-123）、`sub_process_enter_point.init_process_info`（:124）、`@xdeditor_startup.modules_to_update`（:127，@ 跨库到启动包）、`http_requests.goods`（:165）、`window.editor_local_resource`（:173）、`@common.update`（:360）、`ui.login`（:713、:774）、`map_starter`（:715）、`io_modifier`（:471）、`ui/window/plugin`（:473-477）、`trigger.trigger_manager`/`project_manager`（:479-480）、`config.preferences.guide_config`（:684）、`window.autotest_app`/`window.record_player`（:635、:641）、`@common.base.upload_log`（:906、:912）。
- 补丁相关：见上方「加载流程主线」。关键全局：`SCE = ImportSCEContext()`（:83）、`_G.generate_cmd`（:47-53）、`_G.editor_resource_dict()`（:131-133）、`_G.update_editor_resource_dict()`（:435-440）、`_G.upload_log`（:912）。可 hook 点：:122（console 之后）/ :471（io_modifier 之前后）/ EVENT.load_map_done / menu_bar callback_map。

## io_modifier.lua
- 用途：**Hook 编辑器 Lua io 写接口，写文件前调 `io.add_skip_watch` 跳过文件监视**（防编辑器自己的写操作触发文件监听回环）。
- 导出：无（执行型脚本，尾行 `print("成功！")` :83）。
- 依赖：无 require；用 C++ 全局 `ImportSCEContext()`（:3）、`GetMainFrame()`（:5）、`io.add_skip_watch/remove_skip_watch`（:15 等，C++ 引擎接口）。
- 补丁相关：加载时机 = `show_editor_main_ui()` 内 `require 'io_modifier'`（main.lua:471），**先于 ui/window/plugin 加载**。被包装的函数与行号：`io.write`（:13-21，失败时 remove_skip_watch）、`io.rename`（:24-37）、`io.remove`（:40-48）、`io.create_dir`（:51-59）、`io.copy`（:62-70）、`io.copy_to_folder`（:73-81）。`map_path` 缓存 + `eventMgr:register_event('on_map_path_changed', ...)` 换图更新（:6-10）。**这是官方自己的 io hook 样板——证明编辑器 state 下可以随意覆盖 io.* 全局函数**；补丁若需再包一层，直接在本文件之后包即可拿到已被 skip_watch 化的原始函数。

## global/init.lua
- 用途：global 目录入口，仅一行。
- 导出：无。
- 依赖：`include 'global.global'`（global/init.lua:1）。
- 补丁相关：main.lua:21/36/118 `include 'global'` 实际经本文件转到 global.global；include 每次重执行（EDITOR/EVENT 表会被重建）。

## global/global.lua
- 用途：注册 `EDITOR`/`EVENT`/`CPP_EVENT`/`RENDER_PATH`/`EFFECT_TILED_SCENE_NAME`/`MAP_TASK_PRIORITY` 全局常量。
- 导出：无导出（全部写全局）。
- 依赖：无。
- 补丁相关：**EVENT 常量全集见上文清单**。`EDITOR.test = true`（:3）常驻。所有事件订阅走 `EDITOR.event_register(EVENT.xxx, fn)`。

## config/init.lua
- 用途：config 目录入口。
- 导出：无。
- 依赖：`include 'config.ui'`（:1）、`include 'config.localizatioin'`（:2）。
- 补丁相关：main.lua:24/39/121 `include 'config'` 的落点。注意目录名拼写就是 `localizatioin`（少一个 t），require 路径必须照抄。

## config/ui/init.lua
- 用途：UI 配置入口。
- 导出：无。
- 依赖：`include 'config.ui.style'`（:1）、`include 'config.ui.theme'`（:2）。
- 补丁相关：无。

## config/ui/style.lua
- 用途：dark 主题样式常量表（字号/颜色/图标路径，图标统一 `'@xdeditor/ui/images/' .. image` :1-3）。
- 导出：`return return_style`（:391，即 style 表，含 dark/light 子表）。
- 依赖：无。
- 补丁相关：样式改色/改图标的统一数据源；profiler、texture_viewer 等多处 `include 'config.ui.style'`。

## config/ui/theme.lua
- 用途：从 @appui 主题系统取 dark/light theme，追加编辑器各模块色并切换 dark 主题。
- 导出：无（执行型）。
- 依赖：`include '@appui'`（:2，@ 跨库 appui 包）。
- 补丁相关：`theme.change_theme(dark_theme_name)`（:36）——**加载本文件即强制切到 dark 主题**；注释 :1「TODO: 删除style.lua」。

## config/localizatioin/init.lua
- 用途：本地化配置入口，仅 `include 'config.localizatioin.localization'`（:1）。
- 导出：无。依赖：见下。补丁相关：无。

## config/localizatioin/localization.lua
- 用途：编辑器内中英文切换与多语言文本表 metatable。
- 导出：`return { set_text_mt = set_text_mt }`（:34-36）；`set_text_mt(texts)` 返回带 `__index` 按当前语言取文本的只读代理（:18-32）。
- 依赖：无 require；写 `EDITOR.languages = {chinese, english}`（:1-4）、`EDITOR.change_language`（:10-15，切换时 `EDITOR.event_notify(EVENT.language_change)` :13）。
- 补丁相关：默认语言 chinese（:6）；`EDITOR.change_language` 是语言切换钩子。

## config/localizatioin/ui.lua
- 用途：菜单/窗口标题等 UI 文本的中英文对照表（menu_file/menu_open_map/menu_save_map… :9-27）。
- 导出：`return localization.set_text_mt(text)`（:108，即按当前语言动态取值的文本代理表）。
- 依赖：`include 'config.localizatioin.localization'`（:1）。
- 补丁相关：改菜单文案可整体替换本表（文本按 key 动态读，改 default_language 即切换）。

## config/preferences/guide_config.lua
- 用途：新手引导默认配置（各编辑器引导页 url + disabled_guide_url + enable 开关）。
- 导出：`return { disabled_guide_url, editor_app, shop, data_editor, trigger_editor, mechanic_editor }`（:1-25）。
- 依赖：无。
- 补丁相关：main.lua:679-695 用 `User/guide_config.json` 覆盖本默认表生成 `EDITOR.guide_config`；`editor_app.enable = false`（:6）默认关闭欢迎引导。

## config/ini_table/_template.lua
- 用途：数编字段 UI 配置模板样例（category/table_key/type/input/number_input/select/slider/vector/array 等控件类型写法）。
- 导出：`return { {...category 数组...} }`（:2）。
- 依赖：无。补丁相关：写新 ini_table 配置的参考样板；注释 :1 说明 select 缺 options 时会回退 enum.ini 枚举。

## config/ini_table/config_map.lua
- 用途：**ini 表名 → UI 配置模块路径的映射表**，数编显示配置的注册中心。
- 导出：`return config_map`（:69，value 为各配置模块 require 结果）。
- 依赖：`pcall(require, 'config.ini_table.xxx')`（:28-30）逐个加载 animation_data/camera/client_buff/common_spell_data/lightning/particle_data/sound_data/spell/unit_data/unit_item（:2-24）。
- 补丁相关：被 `ini/ini_data.lua:2` require；新增数编表配置在此登记。

## config/ini_table/enum.lua
- 用途：ini 枚举定义（长字符串形式 [[...]]，如 UnitData.CollisionType、SpellData.target_type/area_type/cast_type/affect_type），写入地图 default 表。
- 导出：`return enum_ini`（:54，字符串）。
- 依赖：无。
- 补丁相关：枚举文本由 ini/enum.lua 解析成 AST；加枚举直接改字符串。

## config/ini_table/unit_data.lua
- 用途：UnitData 表的数编 UI 字段配置（含 class_is/class_is_not/has_common_spell 等条件显示函数 :2-22）。
- 导出：`return { ... }`（:126）。
- 依赖：无。补丁相关：条件函数模式 `function(self, data) return data.UnitData.UnitClass ~= class end` 可复用。

## config/ini_table/animation_data.lua / camera.lua / client_buff.lua / common_spell_data.lua / lightning.lua / particle_data.lua / sound_data.lua / spell.lua / unit_item.lua
- 用途：对应 ini 表（AnimationData/Camera/ClientBuff/CommonSpellData/Lightning/ParticleData/SoundData/Spell/UnitItem）的数编 UI 字段配置，纯数据表。
- 导出：各 `return { ... }`（行号：animation_data:1、camera:1、client_buff:1、common_spell_data:1、lightning:1、particle_data:1、sound_data:1、spell:36、unit_item:1）。
- 依赖：无（被 config_map.lua 统一 pcall require）。
- 补丁相关：无直接 hook 点；改数编字段显示即改这些表。

## config/ini_table/attributerange.lua
- 用途：属性取值范围配置（metatable 形式）。
- 导出：`return mt`（:8）。
- 依赖：无。补丁相关：无。

## console/init.lua
- 用途：console 入口：键盘监听 + inner 模式调试 UI + 触发编辑器总开关。
- 导出：无。
- 依赖：`include 'console.keyboard'`（:1）、`include '@common.base.argv'`（:2）、argv `inner` 时 `include 'console.inner_ui'`（:3-5）。
- 补丁相关：**`_G.trigger_editor_on = true`（:6），`-disable_trigger_editor` 参数置 false（:7-9）**——utils/event.lua:533 存图时按此开关决定是否存触发编辑器代码。加载时机在 main.lua:25/40/122。

## console/keyboard.lua
- 用途：全局键盘监听，Ctrl+P/S/F 组合键转发为 EDITOR 事件。
- 导出：无。
- 依赖：`base.game:event('按键-松开'/'按键-按下', ...)`（:12、:17，引擎输入事件）。
- 补丁相关：发 `EVENT.control_and_p_press/control_and_s_press/control_and_f_press`（:23、:28、:33）；注释掉的 `shortcutMgr.RELOAD → EDITOR.event_notify('reload')`（:5-8）说明 reload 事件曾计划挂这里（现由 sub_process F1 触发）。

## console/inner_ui.lua
- 用途：inner 模式下的调试工具条 UI（lite_code 执行框、场景操作按钮等，默认含 ShaderLab 示例代码 :54）。
- 导出：无。
- 依赖：`require '@appui'`（:1，@ 跨库）、`@common.base.argv`（:2）、`ImportSCEContext()`（:3）。
- 补丁相关：注册 `EDITOR.event_register('camera_info_change', ...)`（:11-13，字符串事件）；`project_path = 'E:/NE2/'`（:8）硬编码开发机路径残留。

## examples/init.lua
- 用途：调试面板示例集入口：遍历 `_G.debug.debug_panel_list` 生成一排按钮（:8-39）。
- 导出：无。
- 依赖：`require('examples.create_project')`（:10）、`base.ui.panel/label/create`（:14-31）。
- 补丁相关：仅 `argv.has('show_examples')` 时由 main.lua:914-916 加载；`_G.debug.debug_panel_list = {}`（:9）是示例注册表。

## examples/create_project.lua
- 用途：向 debug_panel_list 注册「创建项目/设置Score/获取Score」等 HTTP 接口调试按钮。
- 导出：无。
- 依赖：`@common.base.lobby`（:7）、`@base.base.account`（:8，@ 跨库 client_base）、`base.calc_http_server_address('publisher', 9000)`（:19）。
- 补丁相关：`account.http_request_with_token`（:21）是带 token 调 publisher 服务的标准姿势。

## exception/init.lua
- 用途：异常处理入口：`include 'exception.network'`（:1）+ `include 'exception.process'`（:2）。
- 导出：无。
- 依赖：见下。补丁相关：main.lua:123 `include 'exception'` 的落点（在主流程五连中，登录之前）。

## exception/network.lua
- 用途：entrance 断线检测与「连接服务器失败」弹窗（重试/关闭）；ams 调试日志转发到信息列表。
- 导出：无。
- 依赖：`@common.base.argv`（:1，local_test 时整文件 return）、`include '@common.base.lobby'`（:2）、`require 'ui.components.message_window'`（:10）。
- 补丁相关（**load_map_done 延迟 require menu_bar 模式**）：
  - 顶层只声明 `local menu_bar`（:3）；**`EDITOR.event_register(EVENT.load_map_done, function() menu_bar = require "ui.menu_bar" end)`（:67-69）**——注释 :68「加载完地图才能保证menubar初始化成功，没加载过地图也不用提示保存」。这是**延迟 require 规避循环依赖/加载时序**的官方样板：顶层不 require menu_bar，等 load_map_done 事件再取。
  - 关闭按钮分支：有 menu_bar 走 `menu_bar.exit_editor()`，否则 `GetMainFrame():AppExit()`（:27-32）。
  - lobby 事件：`register_event('断开连接'/'连接错误'/'已连接'/'ams调试log')`（:37、:42、:51、:62）；'ams调试log' 转发 `EDITOR.event_notify(EVENT.add_info_list, 'ams_debug_lua_info', ...)`（:64）。
  - 弹窗节流：timeout_t = 3 秒、timeout_c = 1 次（:4-5），`message_window.has_window()` 防叠弹窗（:13）。

## exception/process.lua
- 用途：启动时检查残留进程。
- 导出：无。
- 依赖：无；`GetMainFrame().CheckResidualProcess` 存在则调用（:1-3，C++ 接口，能力检测式调用）。
- 补丁相关：C++ 功能探测的写法样板（`if GetMainFrame().Xxx then`）。

## guide/init.lua
- 用途：新手引导入口。
- 导出：无。
- 依赖：`include 'guide.first_page'`（:1）——**注意镜像中不存在 guide/first_page.lua**（guide/ 下仅 init.lua/texts.lua/ui/），该 include 在 v160 镜像内无法解析（可能残留或文件缺失，不臆测原因）；`include 'guide.ui'`（:2）。
- 补丁相关：main.lua 中被注释掉的引导注册代码（main.lua:500-527）曾用 `window.guide_window`；当前引导实际由 utils/event.lua:196 `require 'ui.common.guide'.show_guide_ui()` 拉起。

## guide/texts.lua
- 用途：引导 UI 文本中英文对照（win_title/previous_btn_text/next_btn_text/exit_btn_text 等 :4-20）。
- 导出：`return localization.set_text_mt(texts)`（:215）。
- 依赖：`include 'config.localizatioin.localization'`（:1）。
- 补丁相关：无。

## guide/ui/init.lua
- 用途：引导 UI 入口：注册全局绑定并加载新版通用分页容器。
- 导出：`return normal_page`（:6，即 common_page_view_new 的 mt 实例表）。
- 依赖：`require 'guide.ui.bind'`（:2）、`include 'guide.ui.common_page_view_new'`（:3；旧版 common_page_view 被注释 :4-5）。
- 补丁相关：无。

## guide/ui/bind.lua
- 用途：引导 UI 数据绑定声明（单行起头 `local bind = base.ui.bind` :1）。
- 导出：无（被 require 执行注册）。
- 依赖：`base.ui.bind`（引擎 UI 绑定）。
- 补丁相关：无。

## guide/ui/common_page_view.lua
- 用途：旧版引导通用分页容器（类 mt 模式：get_common_ui/get_target_page/show_ui/hide_ui，分页插件列表 plugin_list）。
- 导出：`return mt`（:398）。
- 依赖：`include 'guide.texts'`（:15）。
- 补丁相关：已被 common_page_view_new 取代（guide/ui/init.lua:4 注释），保留兼容。

## guide/ui/common_page_view_new.lua
- 用途：新版引导通用分页容器（白色背景主题，`mt.extra_plugin_list = {}` :19 支持外挂页）。
- 导出：`return mt`（:170）。
- 依赖：`include 'guide.texts'`（:16）。
- 补丁相关：`extra_plugin_list` 是官方预留的外部页面注入点。

## guide/ui/first_page_ui_new.lua
- 用途：新地图向导第一页「模板选择」UI（模板列表、默认/空白模板图、模板目录遍历 `app.get_resource_dirs()` :39）。
- 导出：`return mt`（:995）。
- 依赖：`include '@common.base.p_ui.btn'`（:2，@ 跨库）、`require 'ini.file_manager'`（:10）、`require '@appui'`（:11）、`include 'guide.texts'`（:30）、`include '@common.base.path'`（:32）、`@common.base.argv`（:38）、`SCE.GetPluginsManager():get_plugin('TileEditor')`（:8）。
- 补丁相关：模板图路径 `@xdeditor/ui/images/template_maps/*.png`（:35-36）。
