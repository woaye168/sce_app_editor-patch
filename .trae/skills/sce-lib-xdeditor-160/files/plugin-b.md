# xdeditor-160 / plugin 批次D 逐文件研究记录（model_editor + obj_editor_cpp + obj_editor_ui + obj_editor_v2 + particle_editor + physic_editor_plugin + sample + tile_editor）

> 研究对象：`D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror\xdeditor-160\plugin\` 下 7 个子目录，共 247 个 .lua。
> 全部结论来自真实读取，关键结论标注行号。源文件部分注释为 GBK 编码（乱码已忽略）。
> 注：`plugin/obj_editor_cpp/` **在镜像中不存在任何 .lua 文件**（旧数编 obj_editor 的 C++ 实现侧；plugin/init.lua:13 的 `include 'plugin.obj_editor'` 对应目录也未在镜像中出现，仅 obj_editor_ui（旧数编 Lua UI 层，仍被大量依赖）与 obj_editor_v2（新数编，默认启用）存在）。

## 总览

- **model_editor**：模型查看/编辑插件的 Lua UI（ModelEditorPlugin 是 C++ 插件，Lua 侧注册 UI + 导入窗口）。
- **obj_editor_ui**：旧数编 UI 层，define/init 聚合模式防循环引用（manager/define.lua:1-8 注释写明约定）；虽旧但**被 gui_editor、bloodstrip_editor、tile_editor、obj_editor_v2 广泛 require**。
- **obj_editor_v2**：新数编核心（默认启用，argv `objv1` 回退旧版），工厂模式管理器，加载/保存挂在 `EVENT.load_map_done/unload_map/save_map_progress_obj_editor`。
- **particle_editor**：粒子编辑器 UI（imgui 风格，appui.ui.basic 组件）。
- **physic_editor_plugin**：物理编辑插件（弹性骨骼&碰撞），仅 argv `inner` 时注册。
- **sample**：**官方 Lua 插件样例**——写 Lua 插件的模板（插件类 + 场景控制器 + UI 注册全流程）。
- **tile_editor**：地编（TileEditor，C++ 插件）的 Lua UI 层，体量最大。

## 关键机制摘录

- **Lua 插件最小骨架**（sample/sample_plugin.lua:6-92）：`class(name, SCE.Plugin)` → 实现 `get_name/get_description/get_category/has_dependencies/get_num_dependencies/get_dependency/is_load/on_pre_load/on_post_load/on_pre_unload/on_post_unload/on_created/on_release` → `SCE.GetPluginsManager():register_plugin(name, Class)`（:92）。`on_pre_load` 里 `register_plugin_ui`（:41），`on_post_load` 里 `base.next` 后 `sceneMgr:show_scene(name, viewport_id)`（:69-72，注释说明必须下一帧否则黑屏）。
- **场景控制器**（sample/sample_scene.lua:8-101）：`class(x, SCE.SceneController)`，生命周期 `on_create_scene/on_destroy_scene/on_show_scene/on_hide_scene/on_update`（:14/:49/:53/:57/:61），`sceneMgr:register_controller(plugin_name, controller)`（:101）；演示 raycast 拾取 + `SCE.MoveControl3D` 拖动物体（:66-96）。
- **数编 v2 生命周期**（obj_editor_v2/init.lua）：`EVENT.obj_init` 通知（:285，各模块响应它向工厂注册预处理）；`EVENT.load_map_done` → `manager.load_data`（:288-330，argv `obj_disable` 可禁用 :290）；`EVENT.unload_map` → `clear_data`（:332-339）；`EVENT.save_map_progress_obj_editor` → `save_data`，通过 `save_map_promise:set_result(0/1)` 汇报（:341-408）；加载/保存完成后 notify `'objv2_loaded'`（:315/:318）/`'objv2_saveded'`（:369/:375）。
- **数编 v2 工厂结构**（obj_editor_v2/init.lua:30-94）：`manager.factory.{module_type,base_type,property_type,node_type,entry_data,ui_data}` 六个工厂；`manager.regist_callback.*_init_callback` 六个注册口，供外部在 `EVENT.obj_init` 时挂扩展。
- **obj_editor_ui 与 v2 的桥**：`manager/define.lua:31-35` 监听 `'objv2_loaded'` 事件拿到 `map_info`（v2 核心对象），`manager.obj_base_interface()`（:39）返回 `map_info.funcs`——**旧 UI 层完全建立在 v2 核心之上**。

---

## plugin/model_editor/init.lua
- 用途：模型编辑器入口。导出：无。依赖：`include 'plugin.model_editor.model_editor_main'`（:1）。补丁相关：无。

## plugin/model_editor/model_editor_main.lua
- 用途：注册 ModelEditorPlugin（C++ 插件）的 Lua UI；argv `inner` 时注册 model_view + bone_tree_view 两个 PluginUI 实例（remove_flag=true），否则注册 inner_model_view。
- 导出：无（执行型）。
- 依赖：`plugin.model_editor.model_view/bone_tree_view`（:2-3）、`@common.base.argv`（:10）、`plugin.model_editor.inner_model_view`（:21）。
- 补丁相关：**PluginUI.new(plugin_name, slot_id) + register_plugin_ui 直接实例注册**的实证（:13-19）；槽位 `'resource_manager_plugin_slot'`（:12）。

## plugin/model_editor/model_view.lua
- 用途：模型查看 UI（PluginUI 子类 `ModelViewUI`，:18）：「模型显示」窗口 dock 到 scene_view_window 右侧（:21-24），组合场景/模型/旋转/动画四个操作组件。
- 导出：`return ModelViewUI`（:198，另有 :200 处条件 return UI 描述表）。
- 依赖：`@appui`（:2）、`config.ui.style`（:3）、`@common.base.util`（:4）、4 个 components（:10-14）、`plugin.plugin_ui`（:16）。
- 补丁相关：无。

## plugin/model_editor/bone_tree_view.lua
- 用途：骨骼树窗口（骨骼/绑点信息表格，socket_configs :9-20）。
- 导出：`return BoneTreeViewUI`（:504，另有 :495 条件 return）。
- 依赖：`@appui`、`window.art_workbench.window_manager.menu_config.menus`（:5）、`components.bone_tree_operation`（:7）。
- 补丁相关：无。

## plugin/model_editor/inner_model_view.lua
- 用途：非 inner 模式的模型查看：监听 `EVENT.create_model` 事件让插件创建模型（:7-8）。
- 导出：`return {...}`（:61，旧式 UI 描述表 {init, ...}）。
- 依赖：SCE（:1-2）。
- 补丁相关：`EVENT.create_model` 是资源树双击模型的通知事件。

## plugin/model_editor/components/anim_operation.lua
- 用途：动画操作组件（播放入口统一为事件 `model_inst_play_animation`，头注释 :1-5）。导出 `anim_operation`（:199）。依赖 `@common.base.p_ui.datagrid`（:8）。补丁相关：无。

## plugin/model_editor/components/anim_period_advanced_operation_v2.lua
- 用途：动画时段高级模式组件（1404 行）。导出 `anim_period_advanced_operation_v2`（:1404）。依赖 `@appui`、`ini.manager`（:3-4）。补丁相关：无。

## plugin/model_editor/components/anim_period_operation.lua
- 用途：动画时段简单模式组件。导出 `anim_period_operation_component`（:1255）。依赖 `ini.manager`、`@appui`、`@common.base.p_ui.checkbox`（:2-4）。补丁相关：无。

## plugin/model_editor/components/bone_tree_operation.lua
- 用途：骨骼树操作组件。导出 `bone_tree_operation`（:98），`base.ui.component('bone_tree_operation')`（:2）。补丁相关：无。

## plugin/model_editor/components/import_operation.lua
- 用途：导入操作组件。导出 `import_operation`（:50）。补丁相关：无。

## plugin/model_editor/components/label_input.lua
- 用途：带提示文本的输入框组件。导出 `label_input`（:65）。补丁相关：无（被多处复用）。

## plugin/model_editor/components/model_operation.lua
- 用途：模型操作组件。导出 `model_operation`（:343）。依赖 `window.art_workbench.res_items.res_data_manager`（:3）、`@common.base`（:5）。补丁相关：无。

## plugin/model_editor/components/progress_bar_materials.lua
- 用途：进度条图标路径常量表（`@xdeditor/ui/images/progress_bar/*`，:3-8）。导出 `local_ui_material`（:28）。补丁相关：无。

## plugin/model_editor/components/rotate_operation.lua
- 用途：单轴旋转组件。导出 `rotate_operation`（:183）。补丁相关：无。

## plugin/model_editor/components/scene_operation.lua
- 用途：场景操作组件。导出 `scene_operation`（:251）。依赖 SCE（:1-2）。补丁相关：无。

## plugin/model_editor/components/select_file_component.lua
- 用途：打开文件组件。导出 `open_file_component`（:167）。补丁相关：无。

## plugin/model_editor/components/select_folder_component.lua
- 用途：打开文件夹组件。导出 `open_folder_component`（:108）。补丁相关：无。

## plugin/model_editor/components/spell_anim_period_operation_component.lua
- 用途：技能预览时间轴组件（简单模式复用 anim_period_operation，高级模式单独实现，:1）。导出 `spell_anim_period_operation_component`（:151）。依赖 `anim_period_operation`/`anim_period_advanced_operation_v2`（:3-4）、`ini.manager`（:6）、`progress_bar_materials`（:8）。补丁相关：被 attribute_editor 复用。

## plugin/model_editor/windows/import_anim.lua
- 用途：导入动画窗口。导出 `{...}`（:235）。依赖 3 个 components（:4-6）、`SCE.GetImporter()`（:10）。补丁相关：`SCE.GetImporter()` 是 C++ 导入器入口。

## plugin/model_editor/windows/import_model.lua
- 用途：导入模型窗口。导出 `{...}`（:218）。依赖 label_input/select_folder_component（:3-4）、`@common.base.p_ui.checkbox`（:1）。补丁相关：无。

## plugin/model_editor/windows/menu.lua
- 用途：模型编辑器弹出菜单（`menu.show_menu(ui)`，:3）。导出 `menu`（:90）。补丁相关：无。

## plugin/model_editor/windows/message_window.lua
- 用途：模型编辑器消息窗口（`message_window.show(ui)`，:6）。导出 `message_window`（:328）。补丁相关：无。

---

## plugin/obj_editor_ui/manager/define.lua
- 用途：旧数编 UI 层 manager 壳：定义 manager/cache 空表（`utils.api_define` 防重复接口），监听 `'objv2_loaded'` 事件桥接 v2 核心（:31-35），`manager.obj_base_interface()` 返回 v2 map_info.funcs（:39-41）。
- 导出：无（由 init.lua return）。
- 依赖：`plugin.obj_editor_ui.tools.init`（:10）。
- 补丁相关：头注释（:1-8）写明 define/init 防循环引用约定——**改这个目录要遵守「只访问目录 init.lua」的规则**。

## plugin/obj_editor_ui/manager/init.lua
- 用途：manager 聚合入口：require define 后依次 require 15 个功能模块（各自向 manager 挂接口）。
- 导出：`return manager`（:17）。
- 依赖：manager/ 下全部模块（:1-16）。
- 补丁相关：**全库被 require 最多的模块之一**（`plugin.obj_editor_ui.manager.init`，bloodstrip/gui_editor/tile_editor/obj_editor_v2 都用）。

## plugin/obj_editor_ui/manager/combo_items.lua
- 用途：枚举选项（普通/带显隐控制/常量值三类，:1-4）。依赖 manager.define。补丁相关：无。

## plugin/obj_editor_ui/manager/const_config.lua
- 用途：常量配置（const_key→appendable_keys、const_value→appendable_enum 等，:1-3）。补丁相关：无。

## plugin/obj_editor_ui/manager/control_show_script.lua
- 用途：显隐控制脚本。补丁相关：无。

## plugin/obj_editor_ui/manager/copy.lua
- 用途：数编节点复制相关。补丁相关：无。

## plugin/obj_editor_ui/manager/data_interface.lua
- 用途：创建/删除/修改等数据接口及相关事件。补丁相关：数编写操作统一入口。

## plugin/obj_editor_ui/manager/data_retrieval.lua
- 用途：数据检索。补丁相关：无。

## plugin/obj_editor_ui/manager/entry_module_info.lua
- 用途：实例模块信息。补丁相关：无。

## plugin/obj_editor_ui/manager/entry_node_description.lua
- 用途：实例节点的描述文本。补丁相关：无。

## plugin/obj_editor_ui/manager/my_module.lua
- 用途：「我的蓝图」相关。补丁相关：无。

## plugin/obj_editor_ui/manager/node_info.lua
- 用途：节点相关信息。补丁相关：无。

## plugin/obj_editor_ui/manager/prop_and_field.lua
- 用途：属性和字段。补丁相关：无。

## plugin/obj_editor_ui/manager/table_value.lua
- 用途：table 类型值处理。补丁相关：无。

## plugin/obj_editor_ui/manager/tracking.lua
- 用途：埋点统计。补丁相关：无。

## plugin/obj_editor_ui/manager/ui_data.lua
- 用途：UI 操作及相关数据修改（注释：UI 操作接口不检查 undoredo、不封装 group，:1）。补丁相关：无。

## plugin/obj_editor_ui/manager/undo_redo.lua
- 用途：封装 undo/redo 方法。补丁相关：无。

## plugin/obj_editor_ui/resource_entry_node.lua
- 用途：通过资源创建的实例节点（resource_config_map：UNIT 模块的 Game.PackageInfo ↔ .prefab 后缀映射，:10-15）。
- 导出：资源节点相关接口表。依赖 manager.init（:2）。补丁相关：无。

## plugin/obj_editor_ui/trigger_data.lua
- 用途：触发器使用的数编相关数据（可选节点缓存等）。依赖 manager.init（:2）、`project_manager`（:9）。补丁相关：无。

## plugin/obj_editor_ui/test/common_ui.lua
- 用途：common_ui 测试。依赖 `plugin.obj_editor_ui.ui.common.init`（:1，**注意该路径 ui/common/ 不在镜像中**，疑旧路径残留）。补丁相关：无。

## plugin/obj_editor_ui/test/imgui_tools.lua
- 用途：imgui_tools 测试。补丁相关：无。

## plugin/obj_editor_ui/tools/define.lua
- 用途：tools 壳（`utils.api_define({}, 'obj_editor_ui_tools')`，:6），挂 const/imgui_tools/utils。补丁相关：无。

## plugin/obj_editor_ui/tools/init.lua
- 用途：tools 聚合入口 + 通用工具（to_pinyin、obj_error 弹窗等）。
- 导出：tools 表（经 define）。依赖 tools.define/cache_tools/field_path_map_tools/image_path_and_import/package_info（:3-8）、`@appui`（:10）、`third-party.lua-pinyin`（:11）。
- 补丁相关：无。

## plugin/obj_editor_ui/tools/cache_tools.lua
- 用途：缓存工具。补丁相关：无。

## plugin/obj_editor_ui/tools/const.lua
- 用途：数编 UI 层常量（基于 v2 const 扩展）。依赖 `plugin.obj_editor_v2.const`（:2）。补丁相关：无。

## plugin/obj_editor_ui/tools/field_path_map_tools.lua
- 用途：属性路径表维护工具。补丁相关：无。

## plugin/obj_editor_ui/tools/image_path_and_import.lua
- 用途：UI 图片路径（相对/`@包名` 前缀）及导入。补丁相关：无。

## plugin/obj_editor_ui/tools/imgui_tools.lua
- 用途：imgui 工具集。依赖 `ui.window_ui`（:2）。补丁相关：无。

## plugin/obj_editor_ui/tools/ini_parser.lua
- 用途：ini 文件解析器（纯 Lua）。补丁相关：无。

## plugin/obj_editor_ui/tools/package_info.lua
- 用途：地图信息、依赖库信息。补丁相关：无。

## plugin/obj_editor_ui/tools/tile_texture_style_info.lua
- 用途：地表贴图样式信息。补丁相关：无。

## plugin/obj_editor_ui/tools/utils.lua
- 用途：与编辑器无关的纯工具（含 `api_define`、`base.game:event('游戏-更新')` 注册更新回调 :3-5）。补丁相关：**`utils.api_define` 是 define/init 模式的基石**。

## plugin/obj_editor_ui/ui/common_ui/define.lua
- 用途：common_ui 壳（聚合 imgui_tools/utils/appui 引用）。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/init.lua
- 用途：公共 imgui 控件聚合入口。导出 common_ui（:9）。依赖 define + 6 个控件模块（:2-8）。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/changed_flag.lua
- 用途：变更标记控件。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/expand_arrow_imgui.lua
- 用途：展开三角按钮。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/inline_table.lua
- 用途：行内 table 控件。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/link_text.lua
- 用途：链接文本控件。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/min_group_item_imgui.lua
- 用途：加减号组控件。补丁相关：无。

## plugin/obj_editor_ui/ui/common_ui/rmgui_wrapper.lua
- 用途：imgui 对 rmgui 的包装（:1 注释：rmgui 未适配圆角缩放时尽量不封装成 imgui）。补丁相关：无。

## plugin/obj_editor_ui/ui/components/editor_component.lua
- 用途：数编 UI 组件（消息中转）。补丁相关：无。

## plugin/obj_editor_ui/ui/components/editor_title.lua
- 用途：标题菜单栏。补丁相关：无。

## plugin/obj_editor_ui/ui/components/entry_module_tree.lua
- 用途：模块列表树。补丁相关：无。

## plugin/obj_editor_ui/ui/components/entry_node_value.lua
- 用途：节点字段值编辑组件（数编属性面板核心）。依赖 `ui.tools.entry_node_value_tools`（:4）。补丁相关：无。

## plugin/obj_editor_ui/ui/components/filter_bar.lua
- 用途：过滤栏。补丁相关：无。

## plugin/obj_editor_ui/ui/components/node_canvas.lua
- 用途：节点画布。补丁相关：无。

## plugin/obj_editor_ui/ui/components/node_preview.lua
- 用途：节点预览（模型/动画预览，SCE :4）。补丁相关：被 tile_editor_main 引用。

## plugin/obj_editor_ui/ui/components/md_text/md_text_component.lua
- 用途：markdown 文本组件。补丁相关：无。

## plugin/obj_editor_ui/ui/components/md_text/md_text_helper.lua
- 用途：markdown 文本解析辅助。补丁相关：无。

## plugin/obj_editor_ui/ui/components/simple_mode/simple_mode.lua
- 用途：数编简单模式组件。补丁相关：无。

## plugin/obj_editor_ui/ui/components/simple_mode/skill_card.lua
- 用途：技能卡片（简单模式）。补丁相关：无。

## plugin/obj_editor_ui/ui/components/simple_mode/unit_card.lua
- 用途：单位卡片（简单模式）。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/async_component_tools.lua
- 用途：异步组件工具（copy_layout 等）。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/csv_export_tool.lua
- 用途：CSV 导出工具。依赖 `window.window_app`（:1）、`@appui`。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/entry_module_tree_tools.lua
- 用途：物编模块树操作工具。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/entry_node_value_tools.lua
- 用途：多节点合并工具。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/format_tool.lua
- 用途：格式化工具。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/model_animation_tools.lua
- 用途：模型/动画路径工具（anim/res_anim 路径规则注释 :1-3）。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/pop_tools.lua
- 用途：右键菜单/弹出框工具。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/redirect_tool.lua
- 用途：跳转工具（引用窗口 `window.refer_window.refer_window` :2）。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/snapshot_tool.lua
- 用途：快照工具（场景/主窗口截图，SCE + GetMainFrame :1-3）。补丁相关：无。

## plugin/obj_editor_ui/ui/tools/validator_tools.lua
- 用途：数编校验工具。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/define.lua
- 用途：type_ui 壳（聚合 manager/pop_tools/common_ui/imgui_tools/appui 引用）。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/init.lua
- 用途：属性类型/基本类型对应 UI 的聚合入口：require 12 个类型 UI + special_field（:4-17）。
- 导出：type_ui 表。补丁相关：**新增数编类型 UI 在这里登记**。

## plugin/obj_editor_ui/ui/type_ui/blood_strip_relationship.lua
- 用途：血条关系列表 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/ctrl_ui.lua
- 用途：控件类型 UI（头注释误写 unknown ui）。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/method_ui.lua
- 用途：公式/函数/验证器 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/res_select_ui.lua
- 用途：资源选择 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/scene_type_ui.lua
- 用途：场景对象选择 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/socket_select_ui.lua
- 用途：绑点选择 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/spell_anim_and_time.lua
- 用途：技能动画选择 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/target_filter.lua
- 用途：filter 类型工具与 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/target_location.lua
- 用途：target location 类型 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/text_ui.lua
- 用途：多语言文本 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/tile_texture_style_item_ui.lua
- 用途：tile style UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/unknown_ui.lua
- 用途：未知类型兜底 UI。补丁相关：无。

## plugin/obj_editor_ui/ui/type_ui/special_field/entry_node_inherit.lua
- 用途：特殊属性：节点继承。补丁相关：无。

---

## plugin/obj_editor_v2/init.lua
- 用途：**新数编核心入口**。定义 ObjManager_Editor（六工厂 + 六注册口），挂地图加载/卸载/保存事件，`return manager`（:458）。
- 依赖：const/utility/header/progress_bar/map_info（:6-17）、`plugin.localization_manager`（:9）、`project_manager.project_manager`（:11）、`inner_ui.obj_message_window`（:13）、`@common.base.argv`（:16）、type_editor 与 data_manager 六工厂（:35-45）。
- 补丁相关：见上方「关键机制摘录」；`SCE.MAPINFO.notify_load_map`（:110）在 current_map_info 赋值时由 __newindex 触发；`manager:partial_save`（:436-456）部分保存走 `EVENT.save_map_progress_obj_editor`；`util.main_manager = manager`（:277）留全局后门；argv `obj_test`（:414）加载 unit_test。

## plugin/obj_editor_v2/const.lua
- 用途：数编系统常量（类型/枚举/命令行参数/系统配置）。补丁相关：`const.CMD.OBJ_Disable` 等 argv key 定义于此。

## plugin/obj_editor_v2/cpp_func.lua
- 用途：C++ 用接口（WinUI 窗口启用回调等，供 C++ 侧回调 Lua）。依赖 const/utility/obj_editor_v2/obj_editor_ui manager（:4-7）。补丁相关：**这是 C++→Lua 的回调面**，改名需谨慎。

## plugin/obj_editor_v2/header.lua
- 用途：复用结构生成 + EmmyLua 类型别名集中定义。补丁相关：无。

## plugin/obj_editor_v2/map_info.lua
- 用途：地图数据对象工厂（ObjMap_Info：一张图/包的完整数编数据；外部通过 EVENT.reload_obj_data 获知，:4）。依赖 type_editor/data_manager/log_manager 工厂（:10-12）等。补丁相关：无。

## plugin/obj_editor_v2/progress_bar.lua
- 用途：数编加载/保存进度条。补丁相关：无。

## plugin/obj_editor_v2/ui_header.lua
- 用途：UI 用数编接口的事件参数数据结构定义（纯类型/结构）。补丁相关：无。

## plugin/obj_editor_v2/utility.lua
- 用途：数编公共方法（日志分级、内存快照、错误窗口等）。依赖 ini_parser/`ini.ast.escape`/message_window/lua-pinyin/argv（:5-10）。补丁相关：`util.traceback`、`util.info/warn`、`util.snapshot_memory` 高频出现于全目录。

## plugin/obj_editor_v2/config_manager/batch_processing_cache.lua
- 用途：批量处理临时缓存（降重复计算）。补丁相关：无。

## plugin/obj_editor_v2/config_manager/config_info.lua
- 用途：节点配置信息记录（校验类型与数据配置文件）。补丁相关：无。

## plugin/obj_editor_v2/config_manager/data_path.lua
- 用途：数据路径记录（日志数据子级）。补丁相关：无。

## plugin/obj_editor_v2/config_manager/dom.lua
- 用途：配置文件结构模型（定义配置结构 + 检测处理）。补丁相关：无。

## plugin/obj_editor_v2/config_manager/log_manager.lua
- 用途：日志容器（数编校验日志）。补丁相关：无。

## plugin/obj_editor_v2/config_manager/save_info.lua
- 用途：保存信息（need_save 判断、validator dirty），位于 map_info.save_info。补丁相关：无。

## plugin/obj_editor_v2/config_manager/type_config_loader.lua
- 用途：数编类型配置文件统一读取。补丁相关：无。

## plugin/obj_editor_v2/config_manager/type_config_saver.lua
- 用途：数编类型配置文件统一保存。补丁相关：无。

## plugin/obj_editor_v2/const_config/init.lua
- 用途：常量配置管理器（初始化常量配置数据，位于 map_info.const_config）。补丁相关：无。

## plugin/obj_editor_v2/const_config/config/node_type.lua
- 用途：常量配置相关节点类型配置文件（数据）。补丁相关：无。

## plugin/obj_editor_v2/const_config/config/property_type.lua
- 用途：常量配置相关属性类型配置文件（数据）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/init.lua
- 用途：数编数据管理中心工厂（map_info.data_manager）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/entry_data/init.lua
- 用途：实例记录与实例模块管理器（map_info.data_manager.entry_node_manager）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/entry_data/entry_module.lua
- 用途：实例模块数据支持。补丁相关：无。

## plugin/obj_editor_v2/data_manager/entry_data/entry_node.lua
- 用途：实例记录数据支持。补丁相关：无。

## plugin/obj_editor_v2/data_manager/entry_data/obj_entry_data.lua
- 用途：数编实例记录/模块注册（插件初始化自动调用）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/entry_data/preprocessor.lua
- 用途：实例记录/模块有效性预处理器。补丁相关：无。

## plugin/obj_editor_v2/data_manager/ui_data/init.lua
- 用途：UI 数据管理器（unit/spell/buff 等 UI 数据；官方 UI 数据有独立链接基本类型，见 const.SYSTEMCONFIG.LinkTypeList，:3）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/ui_data/obj_ui_data.lua
- 用途：数编 UI 数据注册。补丁相关：无。

## plugin/obj_editor_v2/data_manager/ui_data/preprocessor.lua
- 用途：UI 数据预处理器。补丁相关：无。

## plugin/obj_editor_v2/data_manager/ui_data/ui_data_entry.lua
- 用途：UI 数据支持（实例数据）。补丁相关：无。

## plugin/obj_editor_v2/data_manager/ui_data/ui_data_project.lua
- 用途：UI 数据支持（项目数据）。补丁相关：无。

## plugin/obj_editor_v2/exception/exception_data.lua
- 用途：异常信息（每个数据的故障信息）。补丁相关：无。

## plugin/obj_editor_v2/exception/exception_fix.lua
- 用途：异常修复方法字典。补丁相关：无。

## plugin/obj_editor_v2/exception/exception_message_box.lua
- 用途：异常消息窗口。补丁相关：无。

## plugin/obj_editor_v2/inner_ui/obj_message_window.lua
- 用途：数编消息窗口（map_info.inner_ui.obj_message_window）。补丁相关：无。

## plugin/obj_editor_v2/notify/link_data.lua
- 用途：链接关系信息（数据间引用关系）。补丁相关：无。

## plugin/obj_editor_v2/notify/linked_event/init.lua
- 用途：链接事件聚合（linked_event/linked_arg/linked_group 三工厂，:1-3）。导出 `{...}`（:5）。补丁相关：无。

## plugin/obj_editor_v2/notify/linked_event/linked_arg.lua
- 用途：链接事件参数。补丁相关：无。

## plugin/obj_editor_v2/notify/linked_event/linked_event.lua
- 用途：链接事件（类型/数据变更的连锁响应）。补丁相关：无。

## plugin/obj_editor_v2/notify/linked_event/linked_group.lua
- 用途：链接事件参数组。补丁相关：无。

## plugin/obj_editor_v2/notify/linked_event/modifier.lua
- 用途：配置文件数据修改器（实例值直改不走这里，:3）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/init.lua
- 用途：数编类型编辑器工厂（map_info.type_editor），聚合 base_type/module_type/node_type/property_type 四子工厂（:10+）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/base_type/init.lua
- 用途：基本类型管理器（初始化时注册完毕，运行期不可改；扩展点 EVENT.obj_init 的 init_funcs，:4）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/base_type/format_func.lua
- 用途：基本类型单值格式化方法。补丁相关：无。

## plugin/obj_editor_v2/type_editor/base_type/obj_base_type.lua
- 用途：数编基本类型注册。补丁相关：无。

## plugin/obj_editor_v2/type_editor/module_type/init.lua
- 用途：模块类型管理器（unit/spell/buff 等，map_info.type_editor.module_type_manager）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/module_type/module_type.lua
- 用途：模块类型数据支持。补丁相关：无。

## plugin/obj_editor_v2/type_editor/module_type/obj_module_type.lua
- 用途：数编模块类型注册。补丁相关：无。

## plugin/obj_editor_v2/type_editor/module_type/preprocessor.lua
- 用途：模块类型配置预处理器（兼容性升级+配置检测）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/init.lua
- 用途：节点类型管理器。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/binding_func.lua
- 用途：节点类型元素绑定（保存时按绑定对象数据生成当前值）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/node_type.lua
- 用途：节点类型数据支持。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/obj_node_type.lua
- 用途：数编节点类型注册（XDEditor/包/地图三级来源，:3）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/preprocessor.lua
- 用途：节点配置预处理器。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/special_node_format.lua
- 用途：实例记录特殊格式化（整节点级）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/node_type/config/node_type.lua
- 用途：节点类型配置文件（数据，DisplayName 等字段说明 :3-4）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/init.lua
- 用途：属性类型管理器。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/auto_generator.lua
- 用途：属性类型自动生成（目前仅复合 NodeType 筛选 Link 属性类型，:3）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/obj_property_type.lua
- 用途：数编属性类型注册（XDEditor/包/地图三级）。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/preprocessor.lua
- 用途：属性类型配置预处理器。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/property_type.lua
- 用途：属性类型数据支持。补丁相关：无。

## plugin/obj_editor_v2/type_editor/property_type/config/special_property_type.lua
- 用途：特殊属性类型配置文件（数据）。补丁相关：无。

## plugin/obj_editor_v2/undo_redo_manager/init.lua
- 用途：撤销重做管理器（map_info.undo_redo_manager）。补丁相关：无。

---

## plugin/particle_editor/init.lua
- 用途：粒子编辑器入口：注册图片搜索路径（engineres/effect/texture/particle_editor_lib/icon，:6-10），加载主 UI。
- 导出：无。
- 依赖：`@common.base`（:5）、`plugin.particle_editor.ui`（:11）。
- 补丁相关：`pluginManager:register_plugin_ui('ParticlePlugin', ui)` 被注释（:12）——**粒子编辑器 UI 注册当前未启用**（ParticlePlugin 为 C++ 插件，UI 可能由 C++ 侧挂）。

## plugin/particle_editor/ui.lua
- 用途：粒子编辑器主 UI（3168 行，发射器列表 + 模块参数面板全家桶）。
- 导出：`return {...}`（:3168）。
- 依赖：`@appui`（:1）+ 本目录全部面板组件（:4-27+）。
- 补丁相关：无。

## plugin/particle_editor/modules.lua
- 用途：一行 `return modules`（:1）——**`modules` 是外部注入的全局**（本文件内无定义，疑 C++ 或它处预置的粒子模块定义表）。
- 补丁相关：该全局来源未在镜像中取证到。

## plugin/particle_editor/particle_defines.lua
- 用途：粒子模块名常量（CEParticleModuleRequired/Spawn/Lifetime/Size...，:6-11）。
- 导出：`return {...}`（:86）。依赖 SCE（:1）。补丁相关：无。

## plugin/particle_editor/scene_view.lua
- 用途：粒子预览场景控制器（`class('scene_controller', SCE.SceneController)`，:9）。导出 `scene_view`（:132）。依赖 SCE sceneMgr/inputMgr（:2-3）、`@appui`。补丁相关：无。

## plugin/particle_editor/scene_bar.lua
- 用途：粒子场景工具栏。导出 `{...}`（:219）。依赖 `@appui`、SCE eventMgr（:1-3）。补丁相关：无。

## plugin/particle_editor/checkbox.lua
- 用途：复选框组件（`base.ui.component('checkbox', ui_basic)`，:5）。导出 checkbox（:65）。补丁相关：无。

## plugin/particle_editor/color_input_panel.lua
- 用途：颜色输入面板。导出 color_input_panel（:697）。补丁相关：无。

## plugin/particle_editor/color_vec.lua
- 用途：颜色向量编辑组件。导出 color_vec（:111）。补丁相关：无。

## plugin/particle_editor/distribution_float_panel.lua
- 用途：float 分布曲线面板。导出 distribution_float_panel（:183）。补丁相关：无。

## plugin/particle_editor/distribution_vector_panel.lua
- 用途：vector 分布曲线面板。导出 distribution_vector_panel（:229）。补丁相关：无。

## plugin/particle_editor/editable_label.lua
- 用途：可编辑标签组件。导出 editable_label（:96）。补丁相关：无。

## plugin/particle_editor/emitter_item_view.lua
- 用途：发射器列表项视图。导出 simple_list_item（:246）。补丁相关：无。

## plugin/particle_editor/image_picker.lua
- 用途：贴图选择器。导出 image_picker（:281）。补丁相关：无。

## plugin/particle_editor/image_prop_panel.lua
- 用途：图片属性面板。导出 image_prop_panel（:292）。补丁相关：无。

## plugin/particle_editor/list_view.lua
- 用途：通用列表视图。导出 `{...}`（:248）。补丁相关：无。

## plugin/particle_editor/mesh_picker.lua
- 用途：mesh 选择器（`SCE.GetSFbxImport()` :2）。导出 mesh_picker（:587）。补丁相关：无。

## plugin/particle_editor/module_collapse.lua
- 用途：模块折叠面板（`base.ui.component('module_collapse', basic)`，:5）。导出 collapse（:108）。依赖 `@appui.components.basic.basic`（:2）。补丁相关：无。

## plugin/particle_editor/multiple_input_panel.lua
- 用途：多值输入面板。导出 multiple_input_panel（:248）。补丁相关：无。

## plugin/particle_editor/number_input_panel.lua
- 用途：数字输入面板（1299 行）。导出 number_input_panel（:1299）。补丁相关：无。

## plugin/particle_editor/popup_panel_trigger.lua
- 用途：弹出面板触发器组件。导出 popup_panel_trigger（:123）。补丁相关：无。

## plugin/particle_editor/popup_window.lua
- 用途：弹出窗口。导出 popup_window（:86）。补丁相关：无。

## plugin/particle_editor/range3d.lua
- 用途：三维范围输入（range3d_setting/range_input）。导出 `{...}`（:200）。补丁相关：无。

## plugin/particle_editor/ui_wrappers.lua
- 用途：UI 包装器集（label/collapse_panel/setting_panel/label_input/vec_input/label_checkbox 等，被 ui.lua :6-11 取用）。导出 `{...}`（:447）。补丁相关：无。

## plugin/particle_editor/utils.lua
- 用途：粒子编辑器工具（undo_redo 集成、文件操作）。导出表（:294）。依赖 SCE undo_redo_mgr（:2）、`plugin.tile_editor.filesystem`（:3）、`utils.utils`（:4）。补丁相关：无。

## plugin/particle_editor/xyz_input_panel.lua
- 用途：XYZ 三轴输入面板。导出 xyz_input_panel（:354）。补丁相关：无。

---

## plugin/physic_editor_plugin/init.lua
- 用途：物理编辑插件入口。导出：无。依赖 `include '.physic_editor_plugin_main'`（:1）。补丁相关：无。

## plugin/physic_editor_plugin/physic_editor_plugin_main.lua
- 用途：声明并注册 PhysicEditorPlugin（`class(plugin_name, SCE.Plugin)`，:6）；**仅 argv `inner` 时注册插件及 UI**（:39-42）。
- 导出：无。依赖 `physic_editor_plugin_ui`（:7）、`@common.base.argv`（:9）。
- 补丁相关：argv `inner` 控制插件是否生效的模式开关实证。

## plugin/physic_editor_plugin/physic_editor_plugin_ui.lua
- 用途：物理编辑器 UI（弹性骨骼&物理碰撞，1333 行）。导出 `{...}`（:1333）。依赖 `plugin.model_editor.components.select_file_component`（:1）、`@common.base.p_ui.checkbox/datagrid`（:4-5）、collision_groups（:15）。补丁相关：无。

## plugin/physic_editor_plugin/physic_editor_collision_groups.lua
- 用途：碰撞分组常量表（未选择/静物/...，:1-9）。导出 `{...}`（:64）。补丁相关：无。

## plugin/physic_editor_plugin/physic_editor_collision_shapes.lua
- 用途：碰撞外形配置（sphere/capsule/box 默认值表 :6-10 + 各外形 UI）。导出 `{...}`（:496）。补丁相关：无。

---

## plugin/sample/init.lua
- 用途：样例入口。导出：无。依赖 `include '.sample_plugin'`（:1）、`'.sample_scene'`（:2）。补丁相关：无。

## plugin/sample/sample_plugin.lua
- 用途：**官方 Lua 插件样例**（注释 :3「可以参照这里写lua插件」）。完整实现 SCE.Plugin 接口 + on_pre_load 注册窗口 UI（appui.ui.window + base.ui.viewport，:44-58）+ on_post_load 次帧 show_scene（:69-72）。
- 导出：无（:92 register_plugin 副作用）。
- 依赖：`@appui`（:1）。
- 补丁相关：**写编辑器补丁内 Lua 插件的第一参考**；注意 dock_type `'right_fix'`（:49）与 viewport id `'sample_viewport'`（:52）配对 show_scene 第二参数（:71）。

## plugin/sample/sample_scene.lua
- 用途：Lua 场景样例：SceneController 全生命周期 + WASD 相机 + 11x11 Box 阵列 + 鼠标 raycast 拾取 + MoveControl3D 拖动。
- 导出：无；**写全局 `_G.sample_scene_controller`**（:102）。
- 依赖：SCE sceneMgr/inputMgr（:6-7）。
- 补丁相关：场景内拾取/操作的标准代码路径；`SCE.RAY_TRIANGLE/SCE.DRAWABLE_ANY`（:75）等枚举。

---

## plugin/tile_editor/init.lua
- 用途：地编 Lua 层入口。导出：无。依赖 `include '.tile_editor_main'`（:1）、`'.right_click.right_click_panel'`（:2）。补丁相关：无。

## plugin/tile_editor/tile_editor_main.lua
- 用途：地编主逻辑（937 行）：**定义全局 `_G.ACMAP_MANAGER`**（:22）、注册 SceneTimeEditor 插件 UI（:55）、挂 save/load/viewport 事件、地图氛围数据自检修复（:168-210）、tiles 升级流程（:212-239）。
- 导出：`return mt`（:937，一个带 bind_events 的 UI 对象，用于「初始化默认地图」对话框）。
- 依赖：`plugin.tile_editor.scene_time_editor/acmap_manager`（:17-18）、`window.info_window.info_window`（:20）、`@appui`（:21）、`plugin.obj_editor_ui.ui.components.node_preview`（:29）、`plugin.obj_editor_ui.manager.init`（:30）、`.resource_entry_node`（:31）、`plugin.obj_editor_ui.ui.type_ui.define`（:32）、`imgui_tree_popup`（:33）、`window.file_monitor_window`（:34）、`temp.scale_deco_and_area`（:68）。
- 补丁相关：大量历史 UI 注册被注释（:36-53）；事件锚点：`EVENT.save_map_progress_tile_editor`（:96）、`EVENT.tile_editor_viewport_show/hide`（:136/:144）、`EVENT.load_map`（:164）、`EVENT.load_map_done`（:168）、eventMgr `'on_map_loaded'`（:212）；`io.add_skip_watch`（:195）写 .lightgroup 时跳过文件监听。

## plugin/tile_editor/acmap_manager.lua
- 用途：acmap 地图文件管理类（`class('AcmapManager')`，:4）：地图文件完整性检查（Collision.dat/HeightData.dat/map.acmap/map.scene_items，:9-12）。
- 导出：`return AcmapManager`（:110）。依赖 `ui.components.message_window`（:5）。补丁相关：实例化全局点见 tile_editor_main.lua:22。

## plugin/tile_editor/attribute_window.lua
- 用途：地编属性窗口（聚合 7+ 个 attribute_panel）。导出 `{...}`（:224）。依赖各 attribute_panel（:2-8+）。补丁相关：无。

## plugin/tile_editor/collision_sight_edit_view.lua
- 用途：碰撞/视野编辑视图。导出 `{...}`（:265）。补丁相关：无。

## plugin/tile_editor/filesystem.lua
- 用途：文件路径小工具（get_filename/get_folder 等）。导出 `{...}`（:36）。依赖 `@common.base.argv`（:1）。补丁相关：无。

## plugin/tile_editor/scene_time_editor.lua
- 用途：场景时间轴窗口（appui.ui.window 'scene_time_editor'，:3-8）。导出 `{...}`（:14）。补丁相关：注册点 tile_editor_main.lua:55。

## plugin/tile_editor/terrain_select_view.lua
- 用途：地形/单位选择视图（1170 行，地编选择面板核心）。导出 `{...}`（:1170，含 `lock_function` 等，被 plugin_api.lua:137 调用）。依赖 style/MainFrame/SCE/project_manager/appui/path/undo_redo/ui_resolution_content（:1-11）。补丁相关：`plugin_api.lock_function` 的实际实现处。

## plugin/tile_editor/ui_resolution_content.lua
- 用途：UI 分辨率预设数据（自由比例/2340x1080 等，:1-8）。导出 `{...}`（:71）。补丁相关：无。

## plugin/tile_editor/ui_templates.lua
- 用途：地编通用 UI 模板（text_panel 等）。导出表（:46）。补丁相关：无。

## plugin/tile_editor/viewport_attribute.lua
- 用途：视口属性视图（200x200 调试红面板，:2-7）。导出 `{...}`（:24）。补丁相关：无。

## plugin/tile_editor/viewport_input_points_name.lua
- 用途：视口输入点名称视图（`ImportSCEContext().GetTileEditorTool()`，:2）。导出 `{...}`（:176）。补丁相关：`GetTileEditorTool()` 是 C++ 地编工具入口。

## plugin/tile_editor/attribute_panel/anchor_attribute_panel.lua
- 用途：锚点属性面板。导出 `{...}`（:758）。补丁相关：无。

## plugin/tile_editor/attribute_panel/area_attribute_panel.lua
- 用途：区域属性面板。导出 `{...}`（:1332）。补丁相关：无。

## plugin/tile_editor/attribute_panel/atmosphere_attribute_panel.lua
- 用途：氛围（光照组/环境球）属性面板（3812 行，全目录最大）。导出 `{...}`（:3812）。依赖 transform_vec3（:5）等。补丁相关：无。

## plugin/tile_editor/attribute_panel/camera_attribute_panel.lua
- 用途：相机属性面板。导出 `{...}`（:2432）。补丁相关：无。

## plugin/tile_editor/attribute_panel/decoration_attribute_panel.lua
- 用途：装饰物属性面板。导出 `{...}`（:1378）。补丁相关：无。

## plugin/tile_editor/attribute_panel/group_attribute_panel.lua
- 用途：组属性面板。导出 `{...}`（:1009）。补丁相关：无。

## plugin/tile_editor/attribute_panel/item_attribute_panel.lua
- 用途：物品属性面板。导出 `{...}`（:1031）。补丁相关：无。

## plugin/tile_editor/attribute_panel/light_attribute_panel.lua
- 用途：光源属性面板。导出 `{...}`（:3182）。依赖 ui_templates/light_utils（:2、:5）等。补丁相关：无。

## plugin/tile_editor/attribute_panel/minimap_attribute_panel.lua
- 用途：小地图属性面板。导出 `{...}`（:707）。补丁相关：无。

## plugin/tile_editor/attribute_panel/muti_attribute_panel.lua
- 用途：多选属性面板。导出 `{...}`（:649）。补丁相关：无。

## plugin/tile_editor/attribute_panel/terrain_attribute_panel.lua
- 用途：地形属性面板。导出 `{...}`（:1234）。补丁相关：无。

## plugin/tile_editor/attribute_panel/unit_attribute_panel.lua
- 用途：单位属性面板。导出 `{...}`（:1811）。依赖 `@common.base.p_ui.checkbox/select`（:1-2）、terrain_select_view、obj_editor_ui manager（:4-5）。补丁相关：无。

## plugin/tile_editor/attribute_panel/component/preview_window.lua
- 用途：属性面板预览小窗。导出 `{show, hide, update_pos}`（:598）。补丁相关：无。

## plugin/tile_editor/attribute_panel/component/rename.lua
- 用途：重命名弹窗（`rename_ui(initial_name, call_back, title, description)`，:5）。导出 rename_ui（:65）。补丁相关：无。

## plugin/tile_editor/attribute_panel/component/transform_vec3.lua
- 用途：位置/旋转/缩放三轴编辑组件（x/y/z 配色 :3）。导出 transform_vec3（:216）。补丁相关：无。

## plugin/tile_editor/attribute_panel/component/utils.lua
- 用途：属性面板工具（check_range 等地编范围校验，:4-6）。导出 `{...}`（:57）。补丁相关：无。

## plugin/tile_editor/create_panel/create_panel.lua
- 用途：地编创建面板主控（聚合地形/区域/美术材质/单位/物品/光源/导航网格/色彩系统 8 个子面板，:5-12）。
- 导出：`{...}`（:984，含 `add_panel`——**plugin_api.add_create_panel 的扩展入口**）。
- 依赖：`@appui.imgui.basic.*`（:1-3）、8 个子面板、SCE 三管理器 + EProgressBar（:13-17）。
- 补丁相关：`create_panel.add_panel(plugin)` 是向地编创建面板注入自定义页签的官方扩展点。

## plugin/tile_editor/create_panel/create_panel_utils.lua
- 用途：创建面板公共绘制工具（place_holder/drop_tri 等）。导出 `{...}`（:1170）。依赖 `@appui.imgui.basic.*`、obj_editor_ui imgui_tools（:8）。补丁相关：无。

## plugin/tile_editor/create_panel/plugin_template.lua
- 用途：创建面板插件模板（imgui 手写封装 ui.imgui_*，:8-15）。导出 `{...}`（:147，含 `add_ui`，被 plugin_api.create_panel_ui 调用）。补丁相关：**地图级插件往地编面板加 UI 的模板**。

## plugin/tile_editor/create_panel/area_panel.lua
- 用途：区域创建面板。导出 `{...}`（:282）。补丁相关：无。

## plugin/tile_editor/create_panel/art_material_panel.lua
- 用途：美术材质面板。导出 `{...}`（:1551）。补丁相关：无。

## plugin/tile_editor/create_panel/color_system_panel.lua
- 用途：色彩系统面板。导出 `{...}`（:1543）。补丁相关：无。

## plugin/tile_editor/create_panel/item_panel.lua
- 用途：物品创建面板。导出 `{...}`（:623）。补丁相关：无。

## plugin/tile_editor/create_panel/light_panel.lua
- 用途：光源创建面板。导出 `{...}`（:553）。补丁相关：无。

## plugin/tile_editor/create_panel/muti_brush_panel.lua
- 用途：多笔刷面板。导出 `{...}`（:973）。补丁相关：无。

## plugin/tile_editor/create_panel/navmesh_panel.lua
- 用途：导航网格面板。导出 `{...}`（:1293）。补丁相关：无。

## plugin/tile_editor/create_panel/terrain_panel.lua
- 用途：地形创建面板。导出 `{...}`（:2244）。补丁相关：无。

## plugin/tile_editor/create_panel/unit_panel.lua
- 用途：单位创建面板。导出 `{...}`（:616）。补丁相关：无。

## plugin/tile_editor/create_panel/store_item/create_panel_item.lua
- 用途：创建面板资源项（商店项）。导出 item_v2（:735）。依赖 `window.art_workbench.component.ui_array.scene_view`（:1）等。补丁相关：无。

## plugin/tile_editor/create_panel/store_item/muti_deco_item.lua
- 用途：多装饰物资源项。导出 item_v2（:506）。补丁相关：无。

## plugin/tile_editor/designer_work_flow/shader_defines.lua
- 用途：shader 定义数据（texture_unit：材质槽→SCE.TU_* 枚举映射 :3-8、parameter_default_type 等）。导出 `{...}`（:357）。补丁相关：被 material_editor 复用。

## plugin/tile_editor/designer_work_flow/shader_parameter_component.lua
- 用途：shader 参数编辑组件（含 style 子表、create_file_dialog/create_5id_texture_dialog 等，被 material_editor 复用）。导出 `{...}`（:1260）。补丁相关：无。

## plugin/tile_editor/designer_work_flow/light_parameter_component.lua
- 用途：光源参数编辑组件。导出 `{...}`（:351）。依赖 shader_parameter_component（:1）。补丁相关：无。

## plugin/tile_editor/right_click/right_click_panel.lua
- 用途：地编右键菜单面板。导出：右键菜单接口表。依赖 `@common.base.gui.component`（:1）、`project_manager.init`（:8）、SCE eventMgr/pluginMgr。补丁相关：由 tile_editor/init.lua:2 加载。

## plugin/tile_editor/select_list_view/area_item_list_view.lua
- 用途：区域项列表视图组件。导出 item_list_view（:299）。补丁相关：无。

## plugin/tile_editor/select_list_view/imgui_tree_popup.lua
- 用途：imgui 树弹窗/消息框（**即 plugin_api 导出的 message_box**，plugin_api.lua:11）。导出 `{...}`（:276）。依赖 atmosphere_attribute_panel（:6）、`@base.base.message_box`（:7，@ 跨库 client_base）。补丁相关：无。

## plugin/tile_editor/select_list_view/node_item_list_view.lua
- 用途：节点项列表视图（`common_ = common` 备份引擎全局 :2）。导出 item_list_view（:317）。补丁相关：无。

## plugin/tile_editor/select_list_view/scene_list_manager.lua
- 用途：场景列表管理器（区域类型表 area_type :8）。导出 `{...}`（:158）。依赖 obj_editor_ui manager（:1）、`@common.json`（:3）。补丁相关：无。

## plugin/tile_editor/select_list_view/unit_item_list_view.lua
- 用途：单位项列表视图。导出 item_list_view（:267）。补丁相关：无。

## plugin/tile_editor/tool/merge_terrain_material.lua
- 用途：地形材质合并工具。导出 `{...}`（:231）。依赖 filesystem（:2）、`@appui`（:3）。补丁相关：无。
