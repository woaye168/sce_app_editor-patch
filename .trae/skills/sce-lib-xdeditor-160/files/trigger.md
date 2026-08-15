# xdeditor-160 / trigger 目录逐文件研究记录

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/trigger/`（97 个 .lua）。
> 全部结论来自真实读取，关键结论标注行号。源文件注释部分 GBK 乱码已忽略。
> 本目录是 **V1 触发编辑器（lua+ 可视化触发器）** 的实现：lua-parser（LPeg AST）+ rule（AST↔UI 转换规则）+ painter/ui（imgui 绘制）+ debug（触发调试器）。

## 加载链与关键机制

- **入口是 `trigger/entry.lua`，不是 init.lua**（本目录无 init.lua）。entry.lua 只做两件事：
  1. `EDITOR.event_register(EVENT.trigger_editor_pre_require, ...)`（entry.lua:6-16）：按 `project_manager.get_trigger_editor_mode()` 分流——V1 加载 `window.trigger_editor_app` / `window.trigger_editor_api_ui_app` / `window.trigger_validator_editor_app` / `trigger.ui.formula_editor`；V2 只加载 `trigger_editor_v2.lua.app.trigger_editor_app`。注释（:1）明确「尽量将v1和v2隔离开」。
  2. `EDITOR.event_register(EVENT.save_map_progress_trigger_editor, ...)`（:19-75）：地图保存时生成依赖库与原生 lua（V1 走 `ui.utils.get_origin_lua_file`，V2 转发 `EVENT.save_map_progress_trigger_editor_v2`），仅在 `_G.trigger_editor_on == true` 时执行（:26）。
- **lua+ 文件标记**：`trigger_manager.lua:46-54` 定义 `read_sign = '--- lua_plus ---'`（文件首行有此标记才会被解析，否则 ignore，:307-323）、`skip_undefined_comment = '- skip_undefined ---'`；保留关键字列表见 `const.lua:159-172`（`--- origin_lua ---` / `--- lua_plus ---` / `----- split -----` / `--[[<\DISABLED\>]]`）。
- **依赖库管理已迁移 V2**：trigger_manager.lua:29-35 从 `trigger_editor_v2.lua.trigger_manager.lib_manager` 引入 LIBS/get_libs/init_map_libs 等；:70-245 大段旧实现被整段注释保留。
- **V1/V2 调试器共存**：`trigger/debug/init.lua:5-6` 同时 require v1 与 v2 的 trigger_manager。

## 根级文件（16 个）

## trigger/entry.lua
- 用途：触发编辑器入口，按 V1/V2 模式预加载对应 app，并挂钩地图保存事件。
- 导出：无（执行型脚本，注册两个 EDITOR 事件回调）。
- 依赖：`ImportSCEContext()`（:2）、`require 'project_manager.init'`（:3）、回调内按需 require window.trigger_editor_app 等（:9-14）、`trigger.trigger_manager`（:23）、`trigger.debug`（:24）、`ui.utils`（:21-22）。
- 补丁相关：**关键加载时机锚点**——`EVENT.trigger_editor_pre_require` 是触发编辑器所有 app 模块的统一加载入口，在此事件回调前后可注入补丁；`_G.trigger_editor_on`（:26）是保存流程的总开关全局。

## trigger/trigger_manager.lua
- 用途：V1 触发编辑器核心管理器：lua+ 文件缓存（read/parse/生成/重命名）、外部修改标记、依赖库信息、断点信息计算。
- 导出：大表（:938-1002）：`read_lua_file / read_lua_file_forced / update_content_by_codestring / update_codestring_by_content / clear_files / remove_lua_file_cache / rename_lua_file` + 转发 trigger_file_manager 的 `is_file_exist / create_lua_file / remove_lua_file / rename_lua_dir / create_lua_dir / remove_lua_dir / save_changes / clear_changes / init`（:958-969）+ lib_manager 转发 `libs / clear_libs / get_libs / get_upload_libs / all_upload_libs / lib_encrypt_map / lib_need_pak_map / get_target_libs / init_map_libs`（:977-985）+ `get_tree_node_text / read_sign / read_sign_comment / hide_lua_plus_comment / global_variable_text('global_variable.lua') / get_reffered_full_links / get_breakpoint_info / check_have_breakpoint / get_ui_line_position / set_current_bk`。
- 依赖：`ImportSCEContext()`（:1）、`include '@common.base.co'`（:4，@ 跨库 script/common）、`trigger.lua-parser.parser`（:5）、`trigger.lua-generator`（:6）、`trigger.lua-parser.change_dealer`（:7）、`trigger.lua-parser.utils`（:8-12）、`trigger.lua-parser.basic_typetree`（:13-15、22）、`trigger.ui_rule_new`（:16-21）、`trigger.ui.update_change`（:23）、`trigger.trigger_file_manager`（:24）、`trigger.trigger_ui_matcher`（:25）、`include '@common.base.util'`（:26）、`ini.object_tree.node_type`（:27）、`trigger_editor_v2.lua.trigger_manager.lib_manager`（:29）、`trigger.debug.config`（:41）。
- 补丁相关：
  - 注册三个 EDITOR 事件：`EVENT.trigger_editor_reload`（:571-580，清空缓存→init_typetree→init_rules→通知 init_tree→通知 `'trigger_editor_loaded'`）、`EVENT.read_lua_file`（:588-590）、`EVENT.update_require_enum`（:596-633）。
  - `EDITOR.utils.get_lib_path_by_name = get_lib_path_by_name`（:680）——**向 EDITOR.utils 挂全局函数**，hook 点。
  - 内存缓存 `read_files`（:37）以 `s_or_c/lib_name/relative_path` 为键（:398）。
  - 断点数据结构存于 AST 的 `file_main_block.bks`（:714-728）。

## trigger/trigger_editor_ui.lua
- 用途：V1 触发编辑器主画布组件（base.ui.component），承载 painter 的双向滚动视图。
- 导出：`trigger_editor_ui`（`base.ui.component('trigger_editor_ui')`，:30、:279）。
- 依赖：`require '@common.base' '@xdeditor.global' '@xdeditor.utils' '@xdeditor.config' '@xdeditor.console'`（:1-5，@ 跨库）、`require '@appui'`（:6）、`@common.base.gui.component / control_util`（:7-8）、`require "lpeglabel"`（:20）、`trigger.trigger_ui_painter`（:228）。
- 补丁相关：组件 props（:46-174）含 `show / ui_tree / ui_name / window_name / message_box / args / file_cache / s_or_c / scroll_start / selected_statement / painter / namespace / is_focus`；painter 缺省时自动 `require "trigger.trigger_ui_painter":new()`（:228）；`on_update` 每帧经 `ui.imgui_begin_view` 驱动 `painter:draw_ui_tree`（:255-277）。**组件实例经 window.trigger_editor_app 持有**（见 entry.lua:34 `get_trigger_editor_component_instance()`）。

## trigger/trigger_file_manager.lua
- 用途：触发文件/目录的增删改名 + 操作回退栈（operator_list）。
- 导出（:161-171）：`is_file_exist / create_lua_file / remove_lua_file / rename_lua_file / create_lua_dir / remove_lua_dir / rename_lua_dir / save_changes / clear_changes / init`。
- 依赖：仅引擎全局 `io.*`（io.read/write/remove/rename/list/create_dir）与 `GetMainFrame():GetMapPath()`（:38）。
- 补丁相关：所有文件操作直接落盘并同时记录**逆操作**到 operator_list（:48、:57、:79、:114、:121），`clear_changes()`（:126-147）逆序回放实现撤销；`copy_to_save_type = {dat, acmap}`（:3-6）这两类文件删除前读入内存。直接调引擎 io，无沙箱——编辑器 state 内 io 完好。

## trigger/const.lua
- 用途：触发编辑器 UI 常量：padding、主题色 THEME、语句位置 STM_LOCATION_DEF、图标表 icon_settings、保留关键字 preserved、运算符优先级 op_ranking。
- 导出：`const` 表（:197）。
- 依赖：无。
- 补丁相关：无（纯数据）。

## trigger/lua-generator.lua
- 用途：AST → 原生 lua 代码生成器（V1 保存产物）。
- 导出：`generator` 表（:1322），关键函数 `generator.tostring`（trigger_manager.lua:465 调用点佐证）。
- 依赖：`trigger.lua-parser.utils`（:10-12）、`basic_typetree`（:13-15）、`trigger.lua-parser.parser`（:17）、`trigger.trigger_ui_matcher`（:18）。
- 补丁相关：模块级开关 `origin_lua / calculate_line / ignore_undefined / keep_inferred_type`（:2-7）。

## trigger/selector_utils.lua
- 用途：搜索弹窗数据筛选小工具，仅 `ui2str`（ui 表转字符串）。
- 导出：`{ ui2str }`（:15-17）。
- 依赖：无。
- 补丁相关：无。

## trigger/test.lua
- 用途：临时测试脚本（require 一堆全局库 + lpeg 实验代码）。
- 导出：未见 return（头部 20 行内无，执行型）。
- 依赖：`@common.base`、`@xdeditor.global/utils/config/console`（:1-5）、`include 'config.ui.style'`（:6）、`lpeglabel`（:17）。
- 补丁相关：无。

## trigger/tracking.lua
- 用途：触发编辑器埋点统计（节点位置分类：公式窗口/局部变量/事件等）。
- 导出：表（:184）。
- 依赖：`project_manager.project_manager`（:1）、`trigger.type_infer_basic`（:2）、`common.get_binary_version()`（:5）。
- 补丁相关：无。

## trigger/trigger_ui_matcher.lua
- 用途：AST 与 ui_rule 模式匹配器（matcher.match_block 等），把解析后的 AST 匹配成 UI 节点树。
- 导出：`matcher`（:539）。
- 依赖：`@common.base`、`@xdeditor.*`（:1-5）、`include 'config.ui.style'`（:6）、`trigger.lua-parser.utils_utils`（:7）、`include "trigger.lua-parser.basic_typetree".indexed_rules`（:18）。
- 补丁相关：trigger_manager.lua:414、:448 在每次读文件/更新代码后调 `matcher.match_block`。

## trigger/trigger_ui_painter.lua
- 用途：V1 触发编辑器绘制器（最大文件，6426 行）：语句/表达式 imgui 绘制、右键菜单、选中态、拖拽。
- 导出：两段 return（:3570 中间表、:6426 `painter`）；trigger_editor_ui.lua:228 佐证 `require ... :new(nil, ui_name, window_name)` 为类用法。
- 依赖：`ImportSCEContext().GetUndoRedoManager()`（:8）、`@appui`、`@appui.imgui`、`@appui.imgui.basic.icon`（:9-11）、`plugin.obj_editor_ui.manager.init`（:12）、`trigger.type_infer_basic`（:13）、`trigger.rule.basic`（:16）、`include '@xdeditor.trigger.ui.trigger'`（:17）、`include 'config.ui.style'`（:18）、`trigger.const`（:19）、`trigger.ui.trigger_type`（:20）、`trigger.lua-parser.basic_typetree`（:24）、`trigger.lua-parser.utils`（:27-30）。
- 补丁相关：painter 实例挂在 trigger_editor_ui 组件 bind 上（trigger_editor_ui.lua:143-154、:228-234），是 V1 画布所有交互行为的落点。

## trigger/trigger_ui_searcher.lua
- 用途：触发编辑器全文搜索（搜索框结果匹配与定位）。
- 导出：表（:387）。
- 依赖：`trigger.trigger_manager`（:1）、`trigger.trigger_ui_matcher`（:2）、`basic_typetree`（:3、6）、`trigger.lua-parser.utils`（:5）、`trigger.type_infer_basic`（:10-17）。
- 补丁相关：无。

## trigger/type_infer_basic.lua
- 用途：类型推导基础定义（UNKNOWN/UNDEFINED/特定值/数列/函数 等类型构造器与注解类型）。
- 导出：大表（:593）。
- 依赖：纯定义（头部为 EmmyLua 注解）。
- 补丁相关：无（被全目录广泛 require 的基础模块）。

## trigger/ui_rule.lua
- 用途：旧版规则索引：按 pattern 顶层 tag 索引各 rule 文件的 ui_rules。
- 导出：`indexed_rules`（:62）。
- 依赖：`include('trigger.rule.' .. file)` 动态加载（:17-19），已加载 basic/技能/Buff 等（:21-24）。
- 补丁相关：注意 trigger_ui_matcher.lua:18 实际用的是 `basic_typetree.indexed_rules` 而非本文件——本文件疑似旧路径残留。

## trigger/ui_rule_basic.lua
- 用途：模式匹配 DSL 定义：N/Tag/Type/S/Op/I/K/Following/Any/Empty 及 C 前缀捕获变体、TriggerCondition。
- 导出：DSL 函数表（:159）。
- 依赖：无。
- 补丁相关：所有 rule/*.lua 的公共基础（rule/ai.lua:1-21 为典型引用样例）。

## trigger/ui_rule_new.lua
- 用途：新版规则管理：enum_define/init_rules/special_rules/get_default_type_node/update_require_rule_with_type 等（1434 行）。
- 导出：大表（:1434），含 trigger_manager 引用的 `enum_define / init_rules / special_rules / get_default_type_node / update_require_rule_with_type / remove_require_rule_with_type`（trigger_manager.lua:16-21 佐证）。
- 依赖：`include '@common.base.argv'`（:1）、`basic_typetree` 全家（:2-30）、`third-party.lua-pinyin`（:3）、`trigger.ui.update_struct_change`（:22-23）、`trigger.lua-parser.utils`（:31）、`change_dealer`（:32）、`include 'trigger.ui_rule_basic'`（:40）。
- 补丁相关：`init_rules()` 在 `EVENT.trigger_editor_reload` 中被调（trigger_manager.lua:577）。

## trigger/ui/ 子目录（13 个）

## trigger/ui/trigger.lua
- 用途：触发器语句/节点的基础 UI 绘制元素库（被 painter include）。
- 导出：表（:2145）。
- 依赖：`ImportSCEContext()`（:1）、`@appui.imgui` 及 basic 组件（:2-5）、`trigger.const`（:6-10）、`type_infer_basic`（:12-13）、`basic_typetree`（:14）。
- 补丁相关：无。

## trigger/ui/trigger_select.lua
- 用途：触发编辑器的搜索/选择弹窗 UI（注释 :1）。
- 导出：表（:1315）。
- 依赖：`@appui`/`appui.imgui`（:2-8）、`type_infer_basic`（:9）、`basic_typetree`（:11-15）。
- 补丁相关：无。

## trigger/ui/trigger_type.lua
- 用途：类型相关 UI 逻辑（类型节点操作，1868 行）。
- 导出：表（:1868）。
- 依赖：`@xdeditor.*`（:1-5）、`include '@xdeditor.trigger.ui.trigger'`（:6）、`third-party.lua-pinyin`（:7）、`include 'config.ui.style'`（:8）、`trigger.const`（:9-10）、`lua-parser.utils`（:11-15）。
- 补丁相关：无。

## trigger/ui/trigger_type_ui.lua
- 用途：imgui 类型控件的 base.ui.component 包装（注释 :11）。
- 导出：`trigger_type_ui` 组件（:12、:82）。
- 依赖：`@appui.imgui`（:7）、`type_infer_basic`（:8）、`trigger.trigger_ui_painter`（:9）。
- 补丁相关：无。

## trigger/ui/update_change.lua
- 用途：UI 操作 → AST 变更提交（update_require / update_node 等）。
- 导出：`updater`（:1100）。
- 依赖：`lua-parser.utils_utils`（:1-4）、`lua-parser.utils`（:6）、`change_dealer`（:7-8、11-12）、`ui_rule_new`（:9）。
- 补丁相关：trigger_manager.lua:23 引入其 `update_require`。

## trigger/ui/update_struct_change.lua
- 用途：结构体（自定义类型）变更的 AST 更新。
- 导出：表（:580），含 `get_type_structs / init_type_builder_map`（ui_rule_new.lua:22-23 佐证）。
- 依赖：`type_infer_basic`（:1-15）。
- 补丁相关：无。

## trigger/ui/search_box_ui.lua
- 用途：imgui 弹窗消息的 base.ui.component 包装（注释 :10）。
- 导出：`search_box_ui` 组件（:11、:234）。
- 依赖：`@xdeditor.*`（:1-5）、`trigger.const`（:6-7）、`@appui`、`@appui.imgui`（:8-9）。
- 补丁相关：无。

## trigger/ui/node_methods.lua
- 用途：节点编辑操作方法集（增删改、撤销重做对接）。
- 导出：`node_methods`（:601）。
- 依赖：`change_dealer`（:1-2）、`lua-parser.utils`（:3-8）、`ui.update_change`（:9）、`type_infer_basic`（:10）、`trigger_ui_matcher`（:11）、`plugin.obj_editor_ui.manager.init`（:12）、`ImportSCEContext().GetUndoRedoManager()`（:13-14）。
- 补丁相关：无。

## trigger/ui/message_box.lua
- 用途：imgui 版消息弹窗绘制函数。
- 导出：`message_box`（:286）。
- 依赖：`@appui`（:1）、`trigger.const`.THEME（:2）、ui.imgui_* 引擎全局（:3-10）。
- 补丁相关：无。

## trigger/ui/message_box_ui.lua
- 用途：message_box 的 base.ui.component 包装（注释 :9）。
- 导出：`message_box_ui` 组件（:10、:117）。
- 依赖：`@appui.imgui`（:7）、`trigger.ui.message_box`（:8）。
- 补丁相关：无。

## trigger/ui/imgui_base.lua
- 用途：imgui 下拉框等基础控件封装。
- 导出：表（:351）。
- 依赖：`@appui.imgui`（:1）。
- 补丁相关：无。

## trigger/ui/formula_editor.lua
- 用途：公式编辑器窗口组件（V1 预加载模块之一，entry.lua:12）。
- 导出：表（:489）；`base.ui.component('formula_editor')`（:7）。
- 依赖：`window.window_app`（:1）、`trigger.trigger_editor_ui`（:2）、`ui.components.window_title`（:3）、`trigger.rule.basic`.formula_rule（:4）、`lua-parser.parser`（:5）、`trigger_ui_matcher`（:6）、`node_methods`（:8）、`@appui.imgui`（:9）、`lua-generator`（:11）、`type_infer_basic`（:12-13）、`lua-parser.utils`（:14）、`common.get_desktop_resolution()`（:16）。
- 补丁相关：**复用 window_app 窗口框架 + window_title 组件**——菜单注册模式与 window/*.lua app 一致（详见 window-a.md）。

## trigger/ui/back_forward_util.lua
- 用途：触发编辑器前进/后退历史栈（注释 :1-2）。
- 导出：`back_forward_util`（:76），方法 `:insert(lib_name, file_name, line)`（:5）。
- 依赖：无。
- 补丁相关：无。

## trigger/debug/ 子目录（6 个，触发调试器）

## trigger/debug/init.lua
- 用途：触发调试器主体：`new_debugger(version)` 工厂，断点/步进/变量监视，v1/v2 双 manager 兼容（1152 行）。
- 导出：表（:1152），含 `new_debugger`（debug_panel.lua:4、entry.lua:24 调用佐证）。
- 依赖：`ImportSCEContext()`（:1）、`SCE.GetEventManager()/GetDebugManager()`（:2-3）、`trigger.debug.json_file`（:4）、`trigger.trigger_manager`（:5）、`trigger_editor_v2.lua.trigger_manager.trigger_manager`（:6）、`trigger.debug.config`（:7-9）、`project_manager.project_manager`（:10）、`SCE.EDITOR_AS_SERVER/EDITOR_AS_CLIENT`（:12-21）、`@common.base.lni_writer`（:24）。
- 补丁相关：`DebugManager = SCE.GetDebugManager()`（:3）是 C++ 调试管理器入口；debugger:save(map_path) 在保存流程被调（entry.lua:44）。

## trigger/debug/config.lua
- 用途：调试器常量 DEBUGGER_STATUS / BREAKPOINT_CHANGE。
- 导出：`{ DEBUGGER_STATUS, BREAKPOINT_CHANGE }`（:15-16）。
- 依赖：无。
- 补丁相关：无。

## trigger/debug/debug_panel.lua
- 用途：调试面板 UI 组件（963 行）。
- 导出：`debug_panel` 组件（:3、:963）。
- 依赖：`@appui`（:2）、`trigger.debug`.new_debugger（:4）、`debug.config`（:5-6）、`variable_list`/`watched_variable_list`（:7-8）、v1/v2 trigger_manager（:9-10）、`project_manager.project_manager`（:11）、`lua-parser.utils`（:12-13）。
- 补丁相关：无。

## trigger/debug/variable_list.lua
- 用途：调试变量列表组件。
- 导出：`variable_list` 组件（:2、:157）。
- 依赖：`@appui`（:1）、`trigger.debug`.new_debugger（:3）。
- 补丁相关：无。

## trigger/debug/watched_variable_list.lua
- 用途：监视变量列表组件（基于 variable_list）。
- 导出：`watched_variable_list` 组件（:2、:292）。
- 依赖：`@appui`（:1）、`variable_list`（:3）、`trigger.debug`.new_debugger（:4）。
- 补丁相关：无。

## trigger/debug/json_file.lua
- 用途：纯 Lua JSON 编解码（调试协议用）。
- 导出：`json`（:281）。
- 依赖：无。
- 补丁相关：无。

## trigger/lua-parser/ 子目录（11 个，LPeg Lua5.3 解析器，改自 Metalua 风格 AST）

## trigger/lua-parser/parser.lua
- 用途：Lua 5.3 LPeg 解析器，产出 Metalua 风格 AST（头注释 :1-20）；trigger 版增加了类型推导挂接。
- 导出：`parser`（:1363），关键 `parser.parse(code, relative_path, lib_name, ignore_type_infer, s_or_c, '')`（trigger_manager.lua:410 调用佐证）。
- 依赖：同目录 scope/validator 等。
- 补丁相关：无。

## trigger/lua-parser/parser-origin.lua
- 用途：未修改的原版解析器备份。
- 导出：`parser`（:476）。
- 依赖：同 parser.lua。
- 补丁相关：无。

## trigger/lua-parser/scope.lua
- 用途：AST 作用域规则处理（头注释 :1-3）。
- 导出：`scope`（:74）。
- 依赖：无。
- 补丁相关：无。

## trigger/lua-parser/pp.lua
- 用途：AST pretty printer（头注释 :1-3）。
- 导出：`pp`（:327）。
- 依赖：无。
- 补丁相关：无。

## trigger/lua-parser/validator.lua
- 用途：AST 校验器（头注释 :1-3）。
- 导出：`{ validate = traverse, syntaxerror = syntaxerror }`（:396）。
- 依赖：`trigger.lua-parser.scope`（:4-10）。
- 补丁相关：无。

## trigger/lua-parser/change_dealer.lua
- 用途：AST 变更提交/检查（change_committer / change_check），可视化编辑回写 AST 的核心。
- 导出：表（:1491）。
- 依赖：`basic_typetree` 全家（:1-12）。
- 补丁相关：`change_committer.ast_commit_replace` 在代码字符串更新 AST 时调用（trigger_manager.lua:445）。

## trigger/lua-parser/utils.lua
- 用途：解析工具函数库（2733 行）：flattype/type_check/deep_copy_tree_node/get_file_main_block/get_project_main_block/get_require_name_from_path/get_scene_nodes 等。
- 导出：大表（:2733）。
- 依赖：`basic_typetree`（:1-12）。
- 补丁相关：无。

## trigger/lua-parser/utils_utils.lua
- 用途：解析工具第二库（get_assignment_node/update_trigger_tree_node 等）。
- 导出：大表（:1567）。
- 依赖：`basic_typetree`（:1-12）。
- 补丁相关：无。

## trigger/lua-parser/basic_typetree/init.lua
- 用途：基础类型树：global_node/type_define_tree/enum_list/indexed_rules/special_rules/custom_events 等全局规则数据中心（618 行导出表）。
- 导出：大表（:618），含 `init_typetree / get_reffered_full_links / global_node / enum_list / indexed_rules` 等（trigger_manager.lua:13-15、22 佐证）。
- 依赖：`type_infer_basic`（:1-17）、`special_type_utils`（:19-20）。
- 补丁相关：**V1 规则数据中心**，`init_typetree()` 在 reload 事件中重建（trigger_manager.lua:576）。

## trigger/lua-parser/basic_typetree/config.lua
- 用途：物编（对象编辑器）类型配置（3010 行，Item_TYPE/Nodetype_FULLLINK 注解 :1-3）。
- 导出：大表（:3010）。
- 依赖：`type_infer_basic`（:4-15）。
- 补丁相关：无。

## trigger/lua-parser/basic_typetree/special_type_utils.lua
- 用途：特殊类型工具（full_link_types/get_enum_root_name/custom_event_name_text 等）。
- 导出：表（:69）。
- 依赖：（见 basic_typetree/init.lua:19 引用）。
- 补丁相关：无。

## trigger/obselete/ 子目录（3 个，已废弃，注意目录名拼写就是 obselete）

## trigger/obselete/type_deducer.lua
- 用途：旧版类型推导器（注释 :4「deducer阶段只是给每个节点存上type」）。**注意其 require 的是 `trigger.lua-parser.types / api_types`（:1-2），这两个文件已不在 lua-parser/ 下**——本目录整体失效。
- 导出：表（:341）。
- 依赖：（悬空，见上）。
- 补丁相关：无。

## trigger/obselete/types.lua
- 用途：旧版类型名列表（variant/void/number/...）。
- 导出：表（:33）。
- 依赖：无。
- 补丁相关：无。

## trigger/obselete/api_types.lua
- 用途：旧版 API 参数类型构造器（函数/特定值 等）。
- 导出：表（:67）。
- 依赖：无。
- 补丁相关：无。

## trigger/rule/ 子目录（48 个，触发器规则定义，统一模式）

**统一模式**（实证：rule/ai.lua:1-37、rule/单位.lua:1-12、rule/basic.lua:1-30）：`include/require 'trigger.ui_rule_basic'` 取 DSL（N/Tag/Type/S/Op/I/K/Following/Any/Empty/A 及 C 前缀捕获变体、TriggerCondition），配合 `trigger.type_infer_basic` 的类型构造器，定义 `ui_rules` 表并 `return { ui_rules = ui_rules }`。加载方：`trigger/lua-parser/basic_typetree`（indexed_rules）与旧版 `trigger/ui_rule.lua:17-24`（`include('trigger.rule.' .. file)`）。规则定义 AST 模式 ↔ 触发编辑器 UI 的双向转换。

特殊文件：
- **rule/basic.lua** —— 核心基础规则库（3569 行），另导出 `formula_rule`（formula_editor.lua:4 引用）。头部注释「配置了ast和ui之间的转换规则」。
- **rule/create.lua** —— 规则动态创建机制（393 行），引用 `ui_rule_new.get_default_index_rule/get_function_rule/get_default_type_node`（:27-30）。
- **rule/rule_builder.lua** —— 规则构造辅助（EmmyLua 生成头注释 :1-5）。
- **rule/base事件封装.lua** —— 事件封装规则，`return {server_only, client_only, trigger}`（:793）。
- **rule/buff.lua / rule/计算.lua / rule/选取器.lua / rule/单位.lua / rule/游戏.lua / rule/测试.lua / rule/简易ai.lua / rule/点.lua / rule/物品.lua / rule/矩形.lua / rule/小地图.lua / rule/线.lua / rule/圆.lua / rule/区域.lua / rule/单位玩家组.lua / rule/技能效果节点.lua / rule/快照.lua / rule/特效.lua / rule/运动.lua / rule/玩家.lua / rule/动画.lua / rule/中途局.lua / rule/ui.lua** —— 各领域 ui_rules 定义（一行一条，模式同上）。
- **rule/ai.lua / rule/技能.lua / rule/技能公式.lua / rule/排序公式.lua / rule/弹道.lua / rule/字符串.lua / rule/外部数据.lua / rule/碰撞.lua / rule/综合.lua / rule/触发器.lua / rule/英雄.lua / rule/算数.lua / rule/计时器.lua / rule/表现.lua / rule/调试.lua / rule/逻辑.lua / rule/音效.lua / rule/镜头.lua / rule/验证器.lua** —— 各领域 ui_rules 定义（这批多数为 35 行空壳或近空壳，ai.lua:32-33 实证 `ui_rules = {}`）。
- **rule/积分.lua** —— 积分规则定义（未见顶层 `^return`，可能缩进书写，未逐行确认）。

补丁相关：rule 文件是**纯数据定义、经 include 动态加载**——新增/修改 rule 文件即可扩展触发器可选动作，是触发器侧最干净的扩展点；无全局写入、无事件注册。
