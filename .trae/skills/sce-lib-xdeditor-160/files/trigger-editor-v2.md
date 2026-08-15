# xdeditor-160 / trigger_editor_v2 目录逐文件研究记录

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/trigger_editor_v2/`（47 个 .lua）。
> **全部是 TypeScriptToLua 生成物**（const.lua:1 有 `Generated with https://github.com/TypeScriptToLua/TypeScriptToLua` 标记）——V2 触发编辑器的 TS 源码编译产物。统一模式：首行 `require("trigger_editor_v2.lua.lualib_bundle")` 取 `__TS__*` 运行时函数，`local ____exports = {}` 收集导出，末尾 `return ____exports`。故本批一律简录。
> 注意：require 路径大小写不统一（`define.Element` / `define.element`、`define.READER` / `define.reader` 混用），Windows 文件系统不敏感才能工作。

## 总体机制

- **加载入口**：`trigger/entry.lua:14`——V2 模式只 `require 'trigger_editor_v2.lua.app.trigger_editor_app'`。
- **窗口与菜单**（app/trigger_editor_app.lua）：窗口用 `base.ui.create_window_app({...})`（:1597，由 win_app_manager.lua:165 提供的全局便捷函数）创建；菜单**直接** `menu_bar.register("模块/触发编辑器", ____exports.show, nil, {guide_register = guider.guide_register, index = 4})`（:1615）——与 V1 走 `EVENT.window_title_bar_register` 事件不同。复用 V1 的 `trigger.ui.search_box_ui`（:19）与 `trigger.debug.debug_panel`（:20）。
- **与 V1 的共享点**：`trigger_manager/lib_manager.lua` 被 V1 `trigger/trigger_manager.lua:29` 直接引用（LIBS/get_libs/init_map_libs 等依赖库管理实际实现已迁到 V2）；`trigger/debug/init.lua:6` 同时持有 v1/v2 manager；menu_bar.lua:32、:127-156 用 v2 manager 做日志→触发器跳转。
- **数据**：`data/init.lua` 的 `DATA`（app 引用 :24-25）是触发元素数据中心；`define/element.lua` 36507 行，是 TriggerElement/FunctionDefine 等全部元素类定义。
- **包名常量**：const.lua:7-13 `__server__/__client__/__common__/__validator__/__formula__`；`STARRED_TRIGGER_JSON_FILE = "starred_trigger.json"`（:14）；`CURRENT_V2_VERSION = 0.9`（:6）。

## 根级

## trigger_editor_v2/decode_type.lua
- 用途：lpeg 类型串解码（头部全为 TS 源码注释，:1-15）。
- 导出：表（:517）。
- 依赖：lpeglabel（注释中）。
- 补丁相关：无。

## trigger_editor_v2/lua/lualib_bundle.lua
- 用途：TypeScriptToLua 运行时库（__TS__Array*/Map/Error/__TS__Class 等，2425 行）。
- 导出：运行时函数表（:2425）。
- 依赖：无。
- 补丁相关：**所有 v2 文件的第一行依赖**，hook 它可影响整个 v2。

## trigger_editor_v2/lua/const.lua
- 用途：v2 常量（版本/包名/文件名/开关，`ARGV_HAS_INNER` :4）。
- 导出：____exports（:376）。
- 依赖：`@common.base.argv`（:3）。
- 补丁相关：无。

## trigger_editor_v2/lua/utils.lua
- 用途：工具函数（getFileTreeNodeRoot/update_file_tree_node，app :26-28 引用）。
- 导出：____exports（:434）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## app/（5 个，应用层）

## trigger_editor_v2/lua/app/trigger_editor_app.lua
- 用途：**V2 触发编辑器主 app**（1718 行）：窗口创建、文件树、画布装配、菜单注册。
- 导出：____exports（:1718），含 `show`（:1615 注册给菜单）。
- 依赖：`ui.menu_bar`（:11）、`ui.components.window_title`（:12）、`ui.editor_ui`（:15-16）、`trigger_manager.trigger_manager`（:17）、`ui.common.guide`（:18）、`trigger.ui.search_box_ui`（:19，@V1）、`trigger.debug.debug_panel`（:20，@V1）、`define.Element`（:21）、`const`（:23）、`data.init`（:24）、`utils`（:26）、`app.trigger_validator_editor_app / trigger_formula_editor_app`（:29-30）、`project_manager.project_manager`（:31）、`trigger.debug`（:32，@V1）、`plugin.obj_editor_ui.manager.init`（:33）、`ui.obj_type_ui_declare`（:34）、`app.trigger_editor_predict`（:38）、`ui.select`（:39）、`@appui.imgui`（:40）。
- 补丁相关：窗口 `base.ui.create_window_app`（:1597）；菜单直注册（:1615）；`widget:create_window` 创建子窗口（:178、:333）。

## trigger_editor_v2/lua/app/trigger_validator_editor_app.lua
- 用途：V2 验证器编辑窗口（861 行）。
- 导出：____exports（:861）。
- 依赖：同 app 家族；窗口 `base.ui.create_window_app`（:433）。
- 补丁相关：无。

## trigger_editor_v2/lua/app/trigger_formula_editor_app.lua
- 用途：V2 公式编辑窗口（328 行）。
- 导出：____exports（:328）。
- 依赖：同 app 家族；窗口 `base.ui.create_window_app`（:289）。
- 补丁相关：无。

## trigger_editor_v2/lua/app/trigger_editor_predict.lua
- 用途：触发输入预测/补全（20 行即导出，极小封装）。
- 导出：____exports（:20）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/app/backward.lua
- 用途：V2 前进/后退历史（BackWard 类，:5-12）；监听 `EVENT.trigger_v2_backward`（:13-15）。
- 导出：____exports（:66）。
- 依赖：EDITOR/EVENT 全局。
- 补丁相关：无。

## trigger_manager/（3 个）

## trigger_editor_v2/lua/trigger_manager/trigger_manager.lua
- 用途：**V2 触发数据管理器**（3527 行）：全局元素表、TS 打印（lastTsPrinterServer/Client :22）、触发↔lua 行号映射。
- 导出：____exports（:3527），含 `getElementByLuaLine / getValidatorPathByLuaLine / getTriggerDebugPanelInfoLocation / generate_intelligence_recommendation`（menu_bar.lua:127-156、:3053 调用佐证）。
- 依赖：`data.init`（:23）、`define.Reader`（:24-25）。
- 补丁相关：menu_bar 调试日志跳转触发器全靠本模块。

## trigger_editor_v2/lua/trigger_manager/lib_manager.lua
- 用途：**依赖库管理实际实现**（555 行）：LIBS 表、check_lib、各库 LIB_INFO（MAP/SERVER_COMMON/SCRIPT/TS 等，:7）。
- 导出：____exports（:555），含 `LIBS / clear_libs / get_libs / get_target_libs / get_upload_libs / all_upload_libs / init_map_libs / lib_encrypt_map / lib_need_pak_map / get_map_lib_info / get_global_default_info`（trigger/trigger_manager.lua:29-35、:977-985 与 editor_ui.lua:14-15 引用佐证）。
- 依赖：`define.READER`（:8-9）、`const`（:10）、`project_manager.project_manager`（:11）。
- 补丁相关：**V1/V2 共用**，改这里同时影响两代触发编辑器的依赖库解析。

## trigger_editor_v2/lua/trigger_manager/time_stamps.lua
- 用途：时间戳/耗时统计。
- 导出：____exports（:291）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## define/（8 个，数据定义层）

## trigger_editor_v2/lua/define/element.lua
- 用途：**全部触发元素类定义**（36507 行，全库最大文件）：TriggerElement/FunctionDefine/Module/Instant/UnionType 等（data/init.lua:9-20 引用佐证）。
- 导出：____exports（:36507）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/triggerelement.lua
- 用途：TriggerElement 相关补充（含 TSPrinter，editor_ui.lua:10-11 引用）。
- 导出：____exports（:1340）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/builder.lua
- 用途：元素构建器（3573 行）。
- 导出：____exports（:3573）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/reader.lua
- 用途：JSON 读取（`readJson`，lib_manager.lua:8-9、trigger_manager.lua:24-25 引用，大小写 READER/Reader 混用）。
- 导出：____exports（:19）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/generator.lua
- 用途：代码/数据生成器。
- 导出：____exports（:208）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/file.lua
- 用途：文件抽象（13 行即导出，极小）。
- 导出：____exports（:13）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/define/flag.lua
- 用途：标志位定义。
- 导出：____exports（:189）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## data/（9 个，静态数据）

## trigger_editor_v2/lua/data/init.lua
- 用途：**v2 数据中心 DATA**（1662 行）：聚合 define 元素类，构建可搜索元素库。
- 导出：____exports（:1662），含 `DATA`。
- 依赖：`define.Element`（:9-20）。
- 补丁相关：无。

## trigger_editor_v2/lua/data/server/{presets,modules,functions,classes}.lua
- 用途：服务端预设/模块/函数/类静态数据（presets :9、functions :28、modules :27、classes :545 导出）。
- 补丁相关：无。

## trigger_editor_v2/lua/data/client/{presets,modules,functions,classes}.lua
- 用途：客户端预设/模块/函数/类静态数据（presets :9、modules :38、functions :9、classes :44 导出）。
- 补丁相关：无。

## ui/（9 个，imgui 绘制层）

## trigger_editor_v2/lua/ui/editor_ui.lua
- 用途：v2 编辑器 UI 组件 `trigger_editor_v2_ui`（:16 由 app 引用），405 行。
- 导出：____exports（:405）。
- 依赖：`@common.base.gui.component`（:4）、`const`（:5）、`@appui.imgui`（:7）、`ui.painter`（:8-9）、`define.TriggerElement`（:10-11）、`ui.components.message_window`（:13）、`trigger_manager.lib_manager`（:14-15）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/painter.lua
- 用途：v2 画布绘制器 `TriggerEditorPainter`（1471 行）。
- 导出：____exports（:1471）。
- 依赖：lualib_bundle。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/widget.lua
- 用途：v2 控件库（1828 行，`:create_window` 被 app :178 调用）。
- 导出：____exports（:1828）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/select.lua
- 用途：选择面板（4441 行，v2 最大 UI 文件）。
- 导出：____exports（:4441）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/type_editor.lua
- 用途：自定义类型编辑器 UI（1869 行）。
- 导出：____exports（:1869）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/type_widget.lua
- 用途：类型控件（149 行）。
- 导出：____exports（:149）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/scroll.lua
- 用途：滚动容器（105 行）。
- 导出：____exports（:105）。
- 补丁相关：无。

## trigger_editor_v2/lua/ui/obj_type_ui_declare.lua
- 用途：物编类型 UI 声明（`get_order/OnChangeValue/OnRemoveModuleType`，app :34-37 引用）。
- 导出：____exports（:735）。
- 补丁相关：无。

## obj_editor_support/（3 个，数编对接）

## trigger_editor_v2/lua/obj_editor_support/obj_editor_support.lua
- 用途：数编（obj_editor）对接层（677 行）。
- 导出：____exports（:677）。
- 补丁相关：无。

## trigger_editor_v2/lua/obj_editor_support/type_builder.lua
- 用途：数编类型构建（961 行）。
- 导出：____exports（:961）。
- 补丁相关：无。

## trigger_editor_v2/lua/obj_editor_support/field_type.lua
- 用途：字段类型定义（56 行）。
- 导出：____exports（:56）。
- 补丁相关：无。

## support/（1 个）

## trigger_editor_v2/lua/support/validator.lua
- 用途：验证支持工具（67 行）。
- 导出：____exports（:67）。
- 补丁相关：无。

## intelligence_recommendation/（6 个，智能推荐）

## trigger_editor_v2/lua/intelligence_recommendation/intelligence_recommendation.lua
- 用途：智能推荐主逻辑（595 行；`generate_intelligence_recommendation` 菜单入口 menu_bar.lua:3051-3055）。
- 导出：____exports（:595）。
- 依赖：`define.Element`（:11）、`const`（:12）、`lib_manager`（:13-14）、`data.init`（:15）。
- 补丁相关：无。

## trigger_editor_v2/lua/intelligence_recommendation/ui.lua
- 用途：推荐 UI（949 行）。导出 ____exports（:949）。
## trigger_editor_v2/lua/intelligence_recommendation/events.lua
- 用途：推荐事件定义（570 行）。导出 ____exports（:570）。
## trigger_editor_v2/lua/intelligence_recommendation/encode.lua / decode.lua
- 用途：推荐数据编码/解码（:196 / :401 导出）。
## trigger_editor_v2/lua/intelligence_recommendation/hash.lua
- 用途：推荐哈希工具（:80 导出）。
