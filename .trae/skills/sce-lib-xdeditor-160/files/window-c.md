# xdeditor-160 / window 其余子目录逐文件研究记录（批次 C）

> 研究对象：`...\xdeditor-160\window\` 下除 art_workbench 外的全部子目录（61 个 .lua）：autotest/ components_lib/ info_window/ mechanism_editor/ project_manager/ refer_window/ resource_store/ ui/。
> 全部简录；依据头部实读 + 组件声明行号。

## autotest/（5 个，自动化测试）

## window/autotest/autotest_ui.lua
- 用途：`autotest_ui` 组件（:2），自动测试主 UI。
- 导出：组件。
- 依赖：`config.ui.style`（:3）、`@appui`（:4）、`ui.components.window_title`（:5）、`autotest_ui_record_list`（:6）、`const`（:7）。
- 补丁相关：无。

## window/autotest/autotest_ui_record_list.lua
- 用途：`autotest_ui_record_list` 组件（:2），测试记录列表。
- 导出：组件。依赖：同上（:3-5）。
- 补丁相关：无。

## window/autotest/const.lua
- 用途：自动测试常量 ERROR_CODE/WORK_STATE（:1-10）。
- 导出：const 表（注意 :1 是全局赋值 `const = {`，非 local）。
- 补丁相关：无。

## window/autotest/utils.lua
- 用途：字符串工具（iscntrl/isprint 等）。
- 导出：`utils`（:1）。
- 补丁相关：无。

## window/autotest/trigger_editor_serialize_utils.lua
- 用途：触发编辑器序列化工具（自动测试录制用）。
- 导出：`utils`（:3）。
- 依赖：`window.autotest_app`（:1）、`trigger.trigger_ui_matcher`（:2）。
- 补丁相关：trigger_editor_app.lua:19 以 `s_util` 引用。

## components_lib/（13 个，控件库展示，inner 专属）

## window/components_lib/init.lua
- 用途：控件库窗口：菜单 `menu_bar.register('工具/控件库', ...)`（:28-30）+ 懒建窗口 `window_app.new('ComponentsLib', ..., '控件库')`（:16）。
- 导出：`show_window`（:32）。
- 依赖：`window.window_app`（:2）、`components_lib.main_view`（:3）、`ui.menu_bar`（:4）。
- 补丁相关：**仅 inner argv 加载**（window/init.lua:14-16）。

## window/components_lib/main_view.lua
- 用途：控件库主视图（`component { ... window_title ... }`，:9）。
- 导出：组件。依赖：`ui.components.window_title`（:3）、`@common.base.gui.component / control_util`（:4-7）。
- 补丁相关：无。

## window/components_lib/components/（11 个）
- appui_button / color_input / color_input_panel / custom_list_view / icons / lanel_panel / new_menu / new_tree / number_input_panel / resource_array / scene_control_panels —— 各控件展示页（按文件名简录）。

## info_window/（17 个，信息列表面板体系）

**统一模式**：`template/info_panel.lua:4` 定义 `info_panel` 基组件（`base.ui.component('info_panel', basic)`）+ `info_panel.new_info_panel(name)` 工厂；各子面板 `local xxx_panel = info_panel.new_info_panel('面板名')`（debug_core_info_panel.lua:9、debug_lua_info_panel.lua:9、editor_info_panel.lua:7、debug_client_info_panel.lua:12、performance_info_panel.lua:9、cheat_info_panel.lua:11）。数据经 `ini.object_tree.event_manager'.get('info_list')`（template/info_list_painter.lua:2）驱动；写入侧是 `EDITOR.event_notify(EVENT.add_info_list, '面板名', {...})`（menu_bar.lua:168-170 等为实证）。

## window/info_window/info_window.lua
- 用途：信息窗口主组件 `info_window`（:6）+ `show_info_window()`（menu_bar.lua:382、:2579 调用佐证）；聚合 7 个子面板（:8-15），性能面板暂未启用（:16-17）。
- 导出：含 show_info_window 的表。
- 依赖：`window.window_app`（:4）、`ui.components.window_title`（:7）、各 *_panel（:8-15）、`@common.base.gui.control_util`（:14）、`const`（:21）。
- 补丁相关：菜单「调试/调试信息面板」（menu_bar.lua:2578-2580）。

## window/info_window/const.lua
- 用途：各面板常量集（EDITOR_INFO/OBJ_EXCEPTION/DEBUG_LUA/DEBUG_CORE/DEBUG_CLIENT/CHEAT/PERFORMANCE + 总 CONST :342）。
- 导出：CONST。依赖 `plugin.obj_editor_v2.const`（:1）。
- 补丁相关：无。

## window/info_window/template/（5 个）
- info_panel.lua：面板基组件（:4）+ new_info_panel 工厂。
- info_list_painter.lua：列表绘制器（:6 起），数据源 event_manager 'info_list'（:2），右键菜单 `ui.resource_tree_tool_ui`（:4）。
- search_template.lua：搜索栏模板。check_box_template.lua：过滤勾选模板。binary_indexed_tree.lua：树状数组（:2，索引用）。

## window/info_window/各面板目录（8 个面板文件 + 2 辅助）
- editor_info/editor_info_panel.lua：编辑器信息面板（:6-7）。
- debug_lua_info/debug_lua_info_panel.lua：Lua 调试日志面板（:6、:9）。
- debug_core_info/debug_core_info_panel.lua：内核日志面板（:6、:9）。
- debug_client_info/debug_client_info_panel.lua：客户端调试面板（:7、:12），依赖 trigger_manager_v2（:8）做日志跳触发器。
- cheat_info/cheat_info_panel.lua：作弊信息面板（:7、:11），inner 相关 argv（:8-9）。
- performance_info/performance_info_panel.lua：性能面板（:6、:9，未启用）。
- ams_debug_lua_info/ams_debug_lua_info_panel.lua：AMS（匹配后台）日志面板。
- obj_exception_info/：obj_exception_info_panel.lua + exception_info_list_painter.lua（:9，数据源 'obj_exception_list' :3）+ recheck_panel.lua（数编异常复查）。

## mechanism_editor/（3 个，预制功能库）

## window/mechanism_editor/mechanism_editor_ui.lua
- 用途：`mechanism_editor_ui` 组件（:7），预制功能库主 UI。
- 导出：组件。
- 依赖：`SCE.GetSceneManager()`（:3）、`art_workbench.component.ui_array.array`（:4）、`store_items`（:5）、`mechanism_container`（:6）、`ui.components.window_title`（:8）、`plugin.obj_editor_ui.manager.init`（:9）、`http_requests.goods`（:10）、`ui.components.message_window`（:11）、`window.editor_download_manager`（:12）。
- 补丁相关：无。

## window/mechanism_editor/mechanism_container.lua
- 用途：`mechanism_container` 组件（:2），带 md 文本的容器。
- 导出：组件。依赖：`plugin.obj_editor_ui.ui.components.md_text.md_text_component`（:3）。
- 补丁相关：无。

## window/mechanism_editor/store_items.lua
- 用途：商店条目数据/下载逻辑。
- 导出：条目表。依赖：`window.editor_local_resource`（:3）、`@common.update.core.local_version`（:4）、`@common.update`（:5）、`window.editor_download_manager`（:6）、`@common.base.lobby`（:7）。
- 补丁相关：无。

## project_manager/（11 个，项目管理器 UI）

## window/project_manager/project_manager_ui.lua
- 用途：项目管理器主 UI，聚合项目/教程/活动三面板（:2-4）。
- 导出：UI 表。依赖：三个 panel（:2-4）。
- 补丁相关：无。

## window/project_manager/project_manager_project_panel.lua
- 用途：项目列表面板（1762+ 行）；**注册菜单「文件/最近打开/清空历史记录」（:1751）与每个历史项目「文件/最近打开/<路径>」（:1762）、「文件/新建装备局」（:1808）**。
- 导出：面板类。
- 补丁相关：菜单直注册模式样例。

## window/project_manager/project_manager_bottom_bar.lua
- 用途：底栏（window/project_manager.lua:4 引用）。
## window/project_manager/project_manager_course_panel.lua / project_manager_activity_panel.lua
- 用途：教程面板 / 活动面板（project_manager_ui.lua:3-4 引用）。
## window/project_manager/project_manager_items.lua
- 用途：项目条目 UI（配色常量 :4-9）。
## window/project_manager/title_template.lua
- 用途：标题栏模板。
## window/project_manager/item_new_project.lua / item_craft.lua / item_activity.lua
- 用途：新建项目 / craft / 活动条目（按文件名简录）。

## refer_window/（2 个，引用查看窗口）

## window/refer_window/refer_window.lua
- 用途：引用窗口组件（`error_window`，:6）+ 窗口管理；模块常量 MODULES（触发编辑器/地图编辑器…，:13-20）。
- 导出：含 show 的表（menu_bar.lua:25、trigger_editor_app.lua:66 引用佐证）。
- 依赖：`window.window_app`（:2）、`ui.components.window_title`（:7）、`refer_list_painter`（:8）。
- 补丁相关：无。

## window/refer_window/refer_list_painter.lua
- 用途：引用列表绘制器（按文件名简录）。

## resource_store/（7 个，社区资源商店）

## window/resource_store/resource_store_app.lua
- 用途：资源商店 app（头注释 :1-3）；**子进程返回消息桩**（:6-25，`ProcessInfo.message_manager.send_message(nil, 'RESOURCE_STORE_APP', 'show')` :18）。
- 导出：app 类（window/init.lua:29 实例化 `_G.RESOURCE_STORE_APP`）。
- 补丁相关：与 art_workbench 同款主/子进程双形态。

## window/resource_store/resource_store_ui.lua
- 用途：资源商店主 UI（:1-3）。
- 导出：UI。依赖：`@common.base.co`（:4）、`ui.components.window_title`（:6）、`art_workbench.component.ui_array.array`（:7）、`resource_store_item`（:8）、`window.ui.filter_ui.filter_label_panel`（:10）。
- 补丁相关：无。

## window/resource_store/resource_store_web_ui.lua
- 用途：商店 web 版 UI（`component 'resource_store_ui' {...}`，:9）。
- 导出：组件。依赖：`@common.base.gui.component`（:1）、`@common.base.account`（:5）、`editor_download_manager`（:6）、`res_path_kit`（:7）。
- 补丁相关：无。

## window/resource_store/resource_store_server.lua
- 用途：商店服务端接口封装（收藏/已购 API 前缀 :8-10）。
- 导出：`resource_store_server`（:6）。依赖：`@common.base.co`（:2）、`SCE.JSON`（:4）。
- 补丁相关：无。

## window/resource_store/resource_store_item.lua / label_show_panel.lua / components/progress_button.lua
- 用途：商品条目 UI / 标签显示面板 / 进度按钮（按文件名简录）。

## ui/filter_ui/（3 个，资源筛选控件）

## window/ui/filter_ui/filter_ui.lua
- 用途：我的资源窗口筛选控件（分类/本地切换、标签过滤、排序、搜索，头注释 :1-7）。
- 导出：组件。依赖：`@common.base.argv`（:8）、`@common.base.gui.component`（:10）。
- 补丁相关：无。

## window/ui/filter_ui/filter_label_panel.lua / filter_label_button.lua
- 用途：标签过滤面板 / 标签按钮（filter_label_panel 被 resource_store_ui.lua:10 引用）。
