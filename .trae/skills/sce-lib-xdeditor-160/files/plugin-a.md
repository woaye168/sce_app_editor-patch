# xdeditor-160 / plugin 批次C 逐文件研究记录（根级 + attribute_editor + bloodstrip_editor + gui_editor + light_edit_ui + localization_manager + make_human_plugin + material_editor）

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/xdeditor-160/plugin/` 根级 7 文件 + 7 个子目录，共 71 个 .lua。
> 全部结论来自真实读取，关键结论标注行号。源文件部分注释为 GBK 编码（乱码已忽略）。

## 插件机制总结（根级文件，重点）

**加载链**：`plugin/init.lua` 是 C++ 执行的入口脚本，依次 `include` 各插件子目录的 init（init.lua:1-10、:17-18），其中 `plugin.obj_editor`（旧数编，argv `objv1` 时启用）与 `plugin.obj_editor_v2`（新数编）二选一（init.lua:12-16）。**全部用 `include`（不走 package.loaded，每次执行），而非 `require`。**

**插件注册模式（Lua 侧）**：
1. 插件本体是 **C++ 插件**（如 `TileEditor`、`AttributePlugin`、`MakeHumanPlugin`、`MaterialPlugin`），Lua 侧通过 `local XxxPlugin = class(name, SCE.Plugin)` 声明派生类，再调 `SCE.GetPluginsManager():register_plugin(name, Class)` 注册（gui_editor/init.lua:26、:295；material_editor/material_plugin.lua:6、:42）。
2. C++ 插件实例方法在 `plugins_manager.lua` 被 **hook 包装**：`_load_plugin = PluginsManager.load_plugin`（:9）/ `_unload_plugin`（:10），重写后的 `load_plugin(name, show_ui)`（:229-239）在调完 C++ 原方法后追加 `load_plugin_ui(name)`——**Lua 插件 UI 体系完全挂在这两个 hook 上**。
3. 插件 UI 注册两种形式：
   - `register_plugin_ui(name, pluginUI)`（:260-267）：注册单个 UI 描述表（旧式：`{name, ui, init, slot_id, remove_flag}`；新式：`plugin_ui.lua` 的 PluginUI 类实例，`is_new_ui=true`，用 `ui_template` + `ui:init(ui, bind)`）。
   - `register_plugin_ui_list(plugin_name, slot_id, remove_flag, ui_list)`（:282-289）：按路径列表 `include` 每个 UI 模块、`PluginUI.new(plugin_name, slot_id)` 实例化后注册（make_human_plugin 走此路）。
4. UI 挂载槽位：默认 `'plugin_attribute_slot'`（:27、:50），各插件可指定（如 material 用 `'resource_manager_plugin_slot'`），通过 `base.ui.create(template, id)` 建 UI 后 `base.ui.map[slot_id]:add_child(ui)` 挂入编辑器主界面（:24-31）。
5. **地图级插件**：`EVENT.load_map_done` 回调（:446-503）把地图目录注册进 PathSearcher（`SCE.Common.set_package_to_path_searcher(package_name, map_path)`，:489），若地图存在 `ui/script/plugin/init.lua` 则 `require('@<package_name>/plugin')` 并调用其 `plugin.load()`，切图时调上次的 `plugin.unload`（:493-500）。`EVENT.add_lib_path` 回调（:373-440）解析地图 `libs.json`/`local_libs.json` 递归把依赖库映射进 PathSearcher（多 API 版本路径优先，:359-368 `get_multi_path`）。
6. `plugin/load.lua`：读命令行 `common.get_argv('load_plugin')`，非空则 `pluginMgr:load_plugin(name)` + `sceneMgr:show_scene(name)`（load.lua:1-22）——**命令行可指定启动时加载哪个插件**。
7. `plugin_api.lua`：暴露给「地图内插件」用的公共 API 表（:395-405），包括血条/单位创建面板接入等，本身不注册插件。
8. `plugins_manager_ui.lua` + `texts.lua`：插件管理器窗口（半成品：`get_plugins()` 返回硬编码假数据 plugins_manager.lua:292-317，勾选回调里 load/unload 被注释 :327-330）。

**关键 C++ 全局（plugin/ 全目录高频出现）**：`ImportSCEContext()`（取 SCE 上下文）、`SCE.GetPluginsManager()`、`SCE.GetSceneManager()`、`SCE.GetUndoRedoManager()`、`SCE.Plugin`（插件基类）、`SCE.SceneController`（场景控制器基类）、`GetMainFrame()`、`EDITOR.event_register/event_notify`、`EVENT.*`、`base.ui.create/base.ui.map`、`log`/`log_file`。

---

## plugin/init.lua
- 用途：plugin 库加载入口，include 拉起全部内置插件模块。
- 导出：无（执行型脚本）。
- 依赖：`include 'plugin.plugins_manager'`（:1）、`plugin.sample`（:2）、`plugin.tile_editor`（:3）、`plugin.make_human_plugin`（:4）、`plugin.model_editor`（:5）、`plugin.load`（:6）、`plugin.attribute_editor`（:7）、`plugin.material_editor`（:8）、`plugin.physic_editor_plugin`（:9）、`plugin.particle_editor`（:10）、`require '@common.base.argv'`（:11）、`plugin.obj_editor` 或 `plugin.obj_editor_v2`（:13/:15，argv `objv1` 分流）、`plugin.gui_editor`（:17）、`plugin.localization_manager`（:18）。
- 补丁相关：**这是 xdeditor 插件体系的总入口，入口插槽/新增插件模块的最佳挂点**（在某行 include 后追加自己的 include 即可随编辑器启动加载）。注意 obj_editor 二选一分支。

## plugin/load.lua
- 用途：命令行 `load_plugin` 参数指定的插件延迟加载 + 场景切换（`base.next` 两帧后 `show_scene`）。
- 导出：无（执行型脚本）。
- 依赖：引擎全局 `common.get_argv`（:1）、`ImportSCEContext()`（:5）、`base.next`（:9、:19）。
- 补丁相关：演示了 `pluginMgr:load_plugin(name)` → `sceneMgr:show_scene(name)` 的插件拉起流程；插件实例方法 `get_name/get_description/get_category/has_dependencies/get_num_dependencies/get_dependency`（:12-17）即 SCE.Plugin 基类接口。

## plugin/plugin_api.lua
- 用途：插件通用 API 集（供地图级插件/其他模块调用）：创建单位面板、锁辅助网格、锁功能等。
- 导出：`{ handel_create_unit, new_scene, lock_assist_grid, lock_function, add_create_panel, add_scene_panel, create_panel_utils, create_panel_ui, message_box }`（:395-405）。`new_scene`（:75-124）与 `add_scene_panel`（:390-392）本体已整体注释，为空实现。
- 依赖：`require 'ini.ast.ast'`（:4）、`ini.object_tree.event_manager`（:5）、`ui.resource_tree_tool_ui`（:7）、`plugin.tile_editor.terrain_select_view`（:8）、`plugin.tile_editor.create_panel.create_panel`（:9）/`.create_panel_utils`（:10）/`.plugin_template`（:12）、`plugin.tile_editor.select_list_view.imgui_tree_popup`（:11）、`@appui.imgui.basic.*`（:142-144，@ 跨库 appui）。
- 补丁相关：`add_create_panel`（:140-383）演示向 TileEditor 的 create_panel 注册自定义面板（`create_panel.add_panel(plugin)`，:382）；`pluginMgr:get_plugin('TileEditor')`（:66、:128、:162）拿 C++ 插件实例调 `set_brush_mode/set_select_mode/set_unit_type` 等——**跨插件调用的标准姿势**。

## plugin/plugin_ui.lua
- 用途：插件 UI 基类 PluginUI（新式插件 UI，`is_new_ui=true`），解决 UI 被不同插件复用/Trigger 移除问题。
- 导出：`return PluginUI`（:49）；`PluginUI:ctor(plugin_name, slot_id)`（:14-25）、`PluginUI:init(ui, bind)`（:28-31）、`PluginUI:set_style()`（:33-48）。
- 依赖：`class('PluginUI', include('ui.base_view'))`（:3）。
- 补丁相关：属性注释（:6-11）列出插件 UI 协议字段：`plugin_name/ui_name/ui_template/slot_id/ui/bind`；子类需设置 `ui_template` 并提供 `init/pre_remove` 等回调（plugins_manager.lua:19-38 的 load_new_ui 流程）。

## plugin/plugins_manager.lua
- 用途：**插件机制核心**。hook C++ PluginsManager 的 load/unload，追加 Lua 插件 UI 生命周期管理；注册地图级 PathSearcher 映射与地图内插件加载。
- 导出：无（执行型脚本，全部改动挂在 C++ 实例方法与事件上）。
- 依赖：`require '@common.json'`（:3）、`@base.update.core.local_api_pak_version`（:4，@ 跨库 client_base）、`@base.update.core.api_version_config`（:5-6）、`include 'plugin.plugins_manager_ui'`（:16）。
- 补丁相关：见上方「插件机制总结」。关键点：hook 点在 :9-10（如需在插件加载前后插桩可再包一层）；`EVENT.add_lib_path`（:373）与 `EVENT.load_map_done`（:446）是切地图时的两个挂点；`try{}` 语法（:468）来自 client_base；`fmt` 全局（:494）为引擎字符串格式化。

## plugin/plugins_manager_ui.lua
- 用途：插件管理器窗口 UI（列表 + 启用勾选 + 保存到地图目录 plugins.json），半成品。
- 导出：`return manager_ui`（:457）；`manager_ui:init()`（:437-455）、`:refresh(plugins)`（:304-333）、`:save_plugins()`（:368-381）、`:load_plugins()`（:383-396）。
- 依赖：`include '@common.base.p_ui.checkbox'`（:2）、`'@common.base.p_ui.select'`（:3）、`include 'config.ui.style'`（:8）、`include 'plugin.texts'`（:9）；引擎全局 `GetMainFrame()`（:6）、`SCE.FWindow`（:446）、`base.ui.create_ui_root/create_to_window`（:450-452）。
- 补丁相关：配置文件 `<地图>/plugins.json`（:11、:360-366）；演示 FWindow + base.ui 建独立窗口的完整流程。

## plugin/texts.lua
- 用途：插件管理器窗口的中英文案表。
- 导出：`return localization.set_text_mt(texts)`（:30），5 个 key（win_title/save_btn_text/close_btn_text/msg_save_success/msg_save_err_no_map）。
- 依赖：`include 'config.localizatioin.localization'`（:1，注意目录名拼写 localizatioin）。
- 补丁相关：`localization.set_text_mt` 是 xdeditor 本库的文案双语 metatable 工具（material_text.lua 同模式）。

## plugin/attribute_editor/init.lua
- 用途：属性编辑器入口，拉起模型查看与贴图查看两块。
- 导出：无。
- 依赖：`include 'plugin.attribute_editor.model_view'`（:1）、`'.texture_view'`（:2）。
- 补丁相关：注意**此处不注册插件类**——`AttributePlugin` 是 C++ 插件，Lua 侧只做事件响应。

## plugin/attribute_editor/model_view.lua
- 用途：响应 `EVENT.create_attribute_unit` 事件，加载 AttributePlugin 插件并按单位数据在场景中显示模型/粒子/prefab。
- 导出：无（执行型脚本，末尾注册事件 :59-61）。
- 依赖：`include 'plugin.attribute_editor.client_unit'`（:6）；`ImportSCEContext()`（:1）。
- 补丁相关：`plugin_manager:load_plugin('AttributePlugin')`（:17）、`unload_plugin_ui('TileEditor')`（:21）、`scene_manager:hide_scene/show_scene`（:22-23）——**属性面板查看单位时会把 TileEditor 场景切走**；插件实例方法 `create_particle/create_model/create_prefab`（:29/:41/:49）为 C++ AttributePlugin 导出。

## plugin/attribute_editor/texture_view.lua
- 用途：监听资源树贴图点击，弹出贴图查看窗口。
- 导出：无（加载即执行 `init()`，:12）。
- 依赖：`include 'ui.res_tree_ctrller'`（:2）；全局 `sce.ui.message_window.show_texture_window`（:8）、`RESOURCES_MANAGER_APP.root_ui`（:8）。
- 补丁相关：`tree_controller:listen_event('on_texture_click', ...)`（:6）——资源树事件挂钩点。

## plugin/attribute_editor/client_unit.lua
- 用途：属性编辑器的场景单位控制器：管理 model_inst 生命周期、注册 `open_Spell/open_AnimationData/...` 等事件、创建技能/动画/Buff/粒子四个查看 UI。
- 导出：`return client_unit`（:413）；关键方法 `init/re_init/remove`（:248/:256/:391）、`view_spell/view_animation/view_particle/view_client_buff`（:166/:187/:203/:224）、`show_range_of_attribute`（:119）。
- 依赖：`require 'ini.manager'`（:1）、`include 'config.ini_table.AttributeRange'`（:2）、4 个 components（:4-7）。
- 补丁相关：UI 槽位 `scene_view_slot_id = 'resource_view'`（:43）；事件 `open_<表名>` 系列（:293-325，表名见 :33-40）、`EVENT.show_unit_by_data`（:268）、`'close_anim_period_operation'`（:327）、`'model_inst_release'`（:407）；`plugin:destroy_model()`（:403）。

## plugin/attribute_editor/components/animation_view_component.lua
- 用途：动画查看组件（播放进度条 + 动画操作）。
- 导出：`return animation_view_component`（:448），`base.ui.component('play_animation_component')`（:2）等。
- 依赖：引擎全局 `base.ui`。
- 补丁相关：无。

## plugin/attribute_editor/components/spell_view_component.lua
- 用途：技能查看组件（内嵌 model_editor 的技能动画时段操作组件）。
- 导出：`return spell_view_component`（:72），`base.ui.component('spell_view_component')`（:6）。
- 依赖：`require 'ini.manager'`（:1）、`include 'plugin.model_editor.components.spell_anim_period_operation_component'`（:2）。
- 补丁相关：无。

## plugin/attribute_editor/components/buff_view_component.lua
- 用途：Buff 查看组件。
- 导出：`return buff_view_component`（:137），`base.ui.component('buff_view_component')`（:3）。
- 依赖：`require 'ini.manager'`（:1）。
- 补丁相关：无。

## plugin/attribute_editor/components/particle_view_component.lua
- 用途：粒子查看组件（通过 model_inst 的 particle_component 播放）。
- 导出：`return particle_view_component`（:54），`base.ui.component('particle_view_component')`（:1）。
- 依赖：无 require。
- 补丁相关：无。

## plugin/bloodstrip_editor/obj_interface.lua
- 用途：血条模板 JSON ↔ 数编对象双向转换（注释：临时 Lua 实现，等 C++ 数编 :1-2）。
- 导出：`return { template_to_obj, obj_to_json_str, ... }`（:481-484 起）。
- 依赖：`require 'plugin.obj_editor_ui.manager.init'`（:3）、`require 'project_manager'`（:4）、`SCE.GetUndoRedoManager()`（:6）。
- 补丁相关：数编节点类型 `$$.template@bloodstrip.BloodStripLayout/BloodStripTemplate`、`$$.bloodstrip.BloodStripTemplate`（:13、:442-448）；末尾注册数编对象树事件回调同步血条编辑器（:478 前 `EDITOR.event_register` 块）；调用 C++ `BloodStripEditor:modify_template/reload_node`（:443-473，全局 BloodStripEditor）。

## plugin/gui_editor/init.lua
- 用途：**GUI 编辑器插件本体**。声明 `GUIEditor = class("GUIEditor", SCE.Plugin)`（:26）并注册（:295）；实现页面/组件的保存、生成代码模板编译、快捷键（复制/粘贴/删除）、数编引用管理。
- 导出：`return {}`（:945，空表——价值全在注册副作用）。
- 依赖：`@common.base.gui.template/component/control_util/package/dump`（:1-13，@ 跨库 common）、`project_manager`（:8）、`trigger.lua-generator`（:11）、`trigger_editor_v2.lua.support.validator`（:12）、`window.info_window.info_window`（:16）、`plugin.obj_editor_ui.manager.init`（:21）、`plugin.gui_editor.format.translator`（:22）、`plugin.gui_editor.tools.image_config`（:23）。
- 补丁相关：**SCE.Plugin 完整接口清单**（:183-235）：`get_name/get_description/get_category/has_dependencies/get_num_dependencies/get_dependency/is_load/on_pre_load/on_post_load/on_pre_unload/on_post_unload/on_created/on_release`——写 Lua 插件照此实现即可。生成物路径常量：UI_SAVE_DIR `/ui/src/gui/page/`（:36）、UI_LOAD_DIR `/ui/script/gui/page/`（:37）、`/ui/script/gui/libs_components.lua`（:40）；`___PRELOADED` 表（:43）用于重载保持状态；`pluginMgr:register_plugin('GUIEditor', GUIEditor)`（:295）是 Lua 注册插件的实证写法。

## plugin/gui_editor/format/component.lua
- 用途：GUI 组件代码生成模板（`{* *}` 占位语法，由 template.compile_string 编译）。
- 导出：`return component '{* type_name *}' {...}`（:5）。
- 依赖：`@common.base.gui.package/component`（:2-3）。
- 补丁相关：头注释「AUTO-GENERATED, MIGHT BE OVERWRITTEN BY GUI-EDITOR」（:1）——该文件本身是生成模板。

## plugin/gui_editor/format/package_init.lua
- 用途：GUI 包 init 代码生成模板。
- 导出：`return pkg.page_pkg(lib_env, {...})`（:3）。
- 依赖：`@common.base.gui.package`（:2）。
- 补丁相关：无。

## plugin/gui_editor/format/template.lua
- 用途：GUI 页面代码生成模板（page_template）。
- 导出：`return gui_pkg.page_template {...}`（:14）。
- 依赖：`@common.base.gui.component/package/on_player_prop/on_unit_prop/ctrl_wrapper`（:2-9）。
- 补丁相关：无。

## plugin/gui_editor/format/translator.lua
- 用途：GUI 编辑器数据 → 生成代码 的翻译器（dump 模板/组件、去 NIL）。
- 导出：`return {...}`（:305）。
- 依赖：`@common.base.gui.package/component/control_util/on_player_prop/on_unit_prop`（:1-6）、`plugin.obj_editor_ui.manager.init`（:2）、`SCE.GetUndoRedoManager()`（:11）。
- 补丁相关：无。

## plugin/gui_editor/tools/gui_tools.lua
- 用途：GUI 编辑器小工具集（deep_copy 带 options 处理器等）。
- 导出：`return {...}`（:57）。
- 依赖：`plugin.obj_editor_ui.manager.init`（:6）。
- 补丁相关：无。

## plugin/gui_editor/tools/image_config.lua
- 用途：GUI 图片属性配置工具（border 编解码、九宫格等）。
- 导出：`return image_config_tools`（:314）。
- 依赖：`plugin.obj_editor_ui.tools.init`（:1）、`plugin.gui_editor.ui.compress_image_window`（:7）、`@appui`（:8）、`@common.base.gui.control_util`（:9）。
- 补丁相关：无。

## plugin/gui_editor/ui/main_view.lua
- 用途：GUI 编辑器主视图（页面树 + 画布 + 拖拽 + 右键菜单 + 触发器联动），2164 行大文件。
- 导出：`return GUIEditorMainView`（:2164）。
- 依赖：`@appui`（:1）、`ui.components.window_title`（:2）、`@common.base.gui.component/control_util`（:3-26）、`plugin.gui_editor.ui.ctrl_table/ctrl_container/ctrl_tree/ctrl_panel`（:27-30）、`@xdeditor.trigger.lua-parser.parser`（:31，@ 跨库自引用）、`ui.components.button_bar/table_form`（:33-34）、`plugin.gui_editor.ui.prop_panel.rename_window`（:36）、`plugin.gui_editor.ui.right_click_panel`（:37）、`SCE.GetEventManager()`（:40）。
- 补丁相关：无（内部 UI）。

## plugin/gui_editor/ui/ctrl_container.lua
- 用途：GUI 控件容器类（编辑器画布中的控件包装，支持选中/拖拽/层级调整），4123 行最大文件。
- 导出：`return CtrlContainer`（:4123）。
- 依赖：`@appui`、`ui.components.color_input`、`@common.base.gui.component/control_util/bibind`、`plugin.gui_editor.ui.ctrl_table/ctrl_border` 等（:1-25+）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_array.lua
- 用途：数组控件创建（按模板递归生成 array 组件）。
- 导出：`return create_array_component`（:92）。
- 依赖：`@common.base.gui.component/control_util`、`plugin.gui_editor.format.translator`、`@appui`（:1-5）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_border.lua
- 用途：控件选中边框组件（紫色高亮框）。
- 导出：`return CtrlBorder`（:73）。
- 依赖：`@common.base.gui.component`、`@appui`（:1-4）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_controller.lua
- 用途：控件八向缩放/锚点控制器（文件头 ASCII 示意图 :1-5）。
- 导出：`return CtrlController`（:383）。
- 依赖：`@appui`、`@common.base.gui.component`（:7-12+）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_panel.lua
- 用途：控件面板（属性区容器）。
- 导出：`return CtrlPanel`（:755）。
- 依赖：`@common.base.gui.component/control_util`、`@appui`（:1-11+）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_table.lua
- 用途：控件表格组件。
- 导出：`return CtrlTable`（:530）。
- 依赖：`@common.base.gui.component/control_util`（:1-12+）。
- 补丁相关：无。

## plugin/gui_editor/ui/ctrl_tree.lua
- 用途：控件树组件（页面结构树）。
- 导出：`return ctrl_tree`（:114）。
- 依赖：`@appui`、`@common.base.gui.component/control_util`（:1-12+）。
- 补丁相关：无。

## plugin/gui_editor/ui/compress_image_window.lua
- 用途：图片压缩选项窗口（32/64/128/256/720 宽度预设，:4-11）。
- 导出：`return {...}`（:203）。
- 依赖：`@appui`（:1）。
- 补丁相关：无。

## plugin/gui_editor/ui/prop_controller.lua
- 用途：属性面板控制器：把多个选中容器的属性操作聚合分发（container_mt 批量调用，:9-20）。
- 导出：`return {...}`（:337）。
- 依赖：`@common.base.gui.table_util`（:1）、`plugin.gui_editor.ui.prop_panel.alignment_prop/size_prop`（:3-4）、`SCE.GetUndoRedoManager()`（:6）。
- 补丁相关：无。

## plugin/gui_editor/ui/right_click_panel.lua
- 用途：GUI 编辑器右键菜单面板。
- 导出：`return show_right_click_panel`（:92）。
- 依赖：`@appui`、`@common.base.gui.component`（:1-8+）。
- 补丁相关：无。

## plugin/gui_editor/ui/search_bar.lua
- 用途：搜索框组件 `component 'SearchBar'`。
- 导出：`return component 'SearchBar' {...}`（:11）。
- 依赖：`@appui`、`@common.base.gui.component`（:1-2）。
- 补丁相关：无。

## plugin/gui_editor/ui/prop_panel/alignment_prop.lua
- 用途：对齐属性面板。导出 `alignment_prop`（:251）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/attribute_binding_prop.lua
- 用途：属性绑定面板（绑定数编字段 + 格式化选项 %.0f 等，:7-8）。导出 `attribute_binding_prop`（:203）。依赖 `plugin.obj_editor_ui.manager.init`（:5）、`@appui`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/color_prop.lua
- 用途：颜色属性面板。导出 `color_prop`（:81）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/ctrl_ninecut_window.lua
- 用途：九宫格切图窗口（拖拽手柄 :8）。导出 `CtrlNinecutWindow`（:688）。依赖 `plugin.gui_editor.tools.image_config`（:3）、`@appui`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/folder_prop.lua
- 用途：文件夹属性面板。导出 `folder_prop`（:143）。依赖 `window.art_workbench.window_manager.menu_config.menus`（:6）、`SCE`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/function_prop.lua
- 用途：函数（事件回调）属性面板。导出 `fn_prop`（:261）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/image_prop.lua
- 用途：图片属性面板。导出 `image_prop`（:236）。依赖 `plugin.gui_editor.tools.image_config`（:2）、`ctrl_ninecut_window`（:3）、`SCE`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/localizable_string_prop.lua
- 用途：多语言文本属性面板。导出 `localizable_string_prop`（:172）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/ninecut_prop.lua
- 用途：九宫格属性面板。导出 `ninecut_prop`（:92）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/particle_prop.lua
- 用途：粒子属性面板。导出 `image_prop`（:93，变量名沿用 image_prop）。依赖 `@common.base.gui.component`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/rename_window.lua
- 用途：重命名窗口。导出 `CtrlRenameWindow`（:190）。依赖 `@appui`、`@common.base.gui.component/control_util`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/size_prop.lua
- 用途：尺寸属性面板。导出 `size_prop`（:176）。依赖 `@common.base.gui.component/control_util`、`@appui`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/spine_anim_or_skin_prop.lua
- 用途：spine 动画/皮肤选择属性（spine 文件 md5/anims/skins 缓存表 :6-8）。导出 `spine_anim_or_skin_prop`（:108）。依赖 `@common.base.gui.component`、`@appui`、`SCE`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/spine_prop.lua
- 用途：spine 资源属性面板（含 import_spine 导入逻辑 :6）。导出 `spine_prop`（:114）。依赖 `spine_anim_or_skin_prop`（:2）、`@appui`、`SCE`。补丁相关：无。

## plugin/gui_editor/ui/prop_panel/transition_prop.lua
- 用途：过渡动画属性面板。导出 `transition_prop`（:446）。依赖 `@common.base.gui.component/control_util`、`@appui`、`SCE`。补丁相关：无。

## plugin/light_edit_ui/light_edit_panel.lua
- 用途：光源编辑面板组件（名称/类型/范围/锥角等参数编辑）。
- 导出：`return light_edit_panel`（:1448），`base.ui.component('light_edit_panel')`（:9）。
- 依赖：`@appui`（:1）、`plugin.light_edit_ui.light_utils`（:2）、`plugin.tile_editor.ui_templates`（:4）、`SCE.GetPluginsManager()/GetUndoRedoManager()`（:6-7）、`@common.base.argv`（:10）。
- 补丁相关：无（TileEditor 光源编辑的 UI 部分）。

## plugin/light_edit_ui/light_list_view.lua
- 用途：光源列表视图组件。
- 导出：`return light_list_view`（:337），`base.ui.component('light_list_view')`（:6）。
- 依赖：`ini.common`（:2）、`@appui`（:4）、`plugin.light_edit_ui.light_utils`（:5）、`SCE.GetEventManager()/GetPluginsManager()`（:11-12）。注意 :1 `common_ = common` 备份引擎全局。
- 补丁相关：无。

## plugin/light_edit_ui/light_manager_view.lua
- 用途：光源管理视图（组合 light_edit_panel，light_list_view 已注释 :16-27）。
- 导出：`return light_manager_view`（:146），`base.ui.component('light_manager_view')`（:7）。
- 依赖：`plugin.light_edit_ui.light_list_view/light_edit_panel`（:1-2）、`SCE.GetSceneManager()/GetPluginsManager()`（:4-5）。
- 补丁相关：无。

## plugin/light_edit_ui/light_utils.lua
- 用途：光源工具：类型枚举 → 中文名。
- 导出：`return { get_light_type_name }`（:13-15）。
- 依赖：`ImportSCEContext()`（:1，`SCE.LIGHT_DIRECTIONAL/LIGHT_SPOT/LIGHT_POINT` :4-8）。
- 补丁相关：无。

## plugin/localization_manager/init.lua
- 用途：多语言管理器：文本/字体两类本地化数据的读写、语言回退链、随地图加载/保存（lua + i18n json 双写）。
- 导出：`return manager`（:444）；`manager.get_text/set_text/delete_text/get_font`（:120/:159/:186/:110）、`set_lang/get_lang/set_editor_lang`（:312/:299/:326）、`get_locales/is_locale/get_locale_display_name`（:285/:265/:255）、`set_other_lang_res/set_obj_editor_map`（:222/:246）。
- 依赖：`@common.base.argv`（:2）、`project_manager`（:3）、`plugin.localization_manager.locale_info`（:4）。
- 补丁相关：存储路径 `/ui/script/obj/localization/%s.lua`（:6）与 `/i18n/%s.json`（:7）；回退链 fallback_map（:18-25）；**加载/保存挂在 `EVENT.localization_on_loadmap`（:409）与 `EVENT.localization_on_savemap`（:421）**——本地化数据的持久化挂点。逻辑注释标明 copy from common/base/localization.lua（:27、:41）。

## plugin/localization_manager/locale_info.lua
- 用途：纯数据：全部 locale 的各语言显示名大表（9118 行）。
- 导出：`return locales`（:9118）。
- 依赖：无。
- 补丁相关：无。

## plugin/localization_manager/unit_test.lua
- 用途：localization manager 的自检用例（set_lang/get_text/set_text 断言）。
- 导出：`return unit_test`（:27），签名 `unit_test(manager)`（:1）。
- 依赖：无。
- 补丁相关：无。

## plugin/make_human_plugin/init.lua
- 用途：捏人插件入口。
- 导出：无。
- 依赖：`include 'plugin.make_human_plugin.make_human_plugin_main'`（:1）。
- 补丁相关：无。

## plugin/make_human_plugin/make_human_plugin_main.lua
- 用途：注册 MakeHumanPlugin（C++ 插件）的三个 Lua UI（复用 model_editor 的 model_view/bone_tree_view + 自己的 main_view）。
- 导出：无（加载即执行 `main()`，:17）。
- 依赖：ui_list 路径（:1-5）：`plugin.model_editor.model_view`、`plugin.model_editor.bone_tree_view`、`plugin.make_human_plugin.make_human_main_view`。
- 补丁相关：**`register_plugin_ui_list` 用法实证**（:14）：`pluginManager:register_plugin_ui_list('MakeHumanPlugin', 'resource_manager_plugin_slot', true, ui_list)`——remove_flag=true 表示卸载时移除 UI。

## plugin/make_human_plugin/make_human_main_ui.lua
- 用途：捏人主窗口 UI 模板（appui.ui.window，标题「捏人系统」:9）。
- 导出：`return make_human_main_window`（:88）。
- 依赖：`@appui`（:1）、`sce.ui.border`。
- 补丁相关：无。

## plugin/make_human_plugin/make_human_main_view.lua
- 用途：捏人主视图（PluginUI 子类，重定向/捏人/换装三页签窗口，dock 到 scene_view_window 右侧 :12-13）。
- 导出：`return MakeHumanMainViewUI`（:153）。
- 依赖：`@appui`（:1）、`@common.base.util`（:2）、`SCE.GetPluginsManager()`（:4）、3 个 components（:6-8）。
- 补丁相关：演示 PluginUI + appui.ui.window + dock_type/dock_target 的新式插件窗口写法。

## plugin/make_human_plugin/make_human_model_list_view.lua
- 用途：捏人模型列表视图（扫 Res 下模型目录、动画 .ani 列表）。
- 导出：`return {...}`（:238）。
- 依赖：`@common.base.util`（:1）、`plugin.make_human_plugin.event`（:2）/`.common`（:3）/`.make_human_model_list_ui`（:7）——**这三个被 include 的文件不在本镜像内**（镜像 make_human_plugin/ 下无 event.lua/common.lua/make_human_model_list_ui.lua，疑为遗漏或未下发）。
- 补丁相关：`io.walk_dir('Res/'..root_path..'/'..model_name..'/Anim', 3)`（:15）——编辑器态直接遍历资源目录。

## plugin/make_human_plugin/components/retarget_component.lua
- 用途：动画重定向组件。导出 `retarget_component`（:330），`base.ui.component('retarget_component')`（:2）。依赖引擎 base.ui。补丁相关：无。

## plugin/make_human_plugin/components/make_human_component.lua
- 用途：捏人操作组件（部件参数调节）。导出 `make_human_component`（:129），内含 `make_human_part_component`（:3）。补丁相关：无。

## plugin/make_human_plugin/components/change_clothes_component.lua
- 用途：换装组件（上衣/裤子部件表 :5-16）。导出 `change_clothes_component`（:224）。补丁相关：无。

## plugin/make_human_plugin/components/import_model_extra.lua
- 用途：捏人（动画集）模型导入 UI（fbx/贴图/保存路径/模型名四项校验 :12-24）。
- 导出：`return {...}`（:147）。
- 依赖：`plugin.model_editor.components.label_input/select_file_component/select_folder_component`（:2-4）、`SCE.GetPluginsManager()`（:7）。
- 补丁相关：无。

## plugin/material_editor/init.lua
- 用途：材质编辑器入口：注册材质插件类 + 场景控制器 + 注册两个插件 UI。
- 导出：无。
- 依赖：`include 'plugin.material_editor.material_plugin'`（:2）、`'.material_scene_ctrl'`（:4）、`'.material_view'`（:6）。
- 补丁相关：**逐个 `register_plugin_ui` 注册**（:11-13），与 make_human 的 register_plugin_ui_list 是两种注册风格。

## plugin/material_editor/material_plugin.lua
- 用途：声明并注册 MaterialPlugin 插件类，构造时把 MaterialSceneCtrl 注册为场景控制器。
- 导出：无（副作用文件，末尾 `SCE.GetPluginsManager():register_plugin(plugin_name, MaterialPlugin)` :42）。
- 依赖：`plugin.material_editor.material_scene_ctrl`（:8）。
- 补丁相关：**`sceneMgr:register_controller(plugin_name, controller)`（:11）是 Lua 侧给插件挂场景控制器的标准写法**；插件方法 `set_material`（:38-40）。

## plugin/material_editor/material_scene_ctrl.lua
- 用途：材质预览场景控制器：`class('MaterialSceneCtrl', SCE.SceneController)`（:5），on_create_scene 建相机（WASD 操作器）、球体模型、raycaster。
- 导出：`return MaterialSceneCtrl`（:103）。
- 依赖：`SCE.GetSceneManager()/GetInputManager()`（:2-3）。
- 补丁相关：**SCE.SceneController 生命周期 `on_create_scene(scene)`（:11）实证**；`SCE.CameraOperatorWASD.new`（:19）、`SCE.ControlRaycaster.new`（:38）、`SCE.GetResource('Material'/'Model', ...)`（:28-29）。

## plugin/material_editor/material_text.lua
- 用途：材质编辑器中英文案表（位置/旋转等参数名）。
- 导出：`return localization.set_text_mt(text)`（:294）。
- 依赖：`include 'config.localizatioin.localization'`（:1）。
- 补丁相关：无。

## plugin/material_editor/material_view.lua
- 用途：材质编辑器主 UI（材质列表 + 属性视图，2137 行；文件对话框/右键菜单/5id/2id 贴图创建对话框）。
- 导出：`return { {name='material_list_view', ui, init, plugin_name='MaterialPlugin', slot_id='resource_manager_plugin_slot'}, {name='material_attribute_view', ...} }`（:2137-2152）——**旧式 UI 描述表数组**。
- 依赖：`@appui`（:1）、`config.ui.style`（:2）、`plugin.tile_editor.designer_work_flow.shader_defines/shader_parameter_component/light_parameter_component`（:3-5）、`plugin.tile_editor.filesystem`（:6）、`ui.res_tree_ctrller`（:7）、`ui.resource_manager`（:8）、`plugin.material_editor.material_text`（:34）。
- 补丁相关：事件 `'change_material_info'`（:2121）与 `ModelEditorPlugin:get_model()`（:2122-2124）联动；argv `inner`（:2130）控制内嵌模式。
