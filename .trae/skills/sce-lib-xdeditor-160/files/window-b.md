# xdeditor-160 / window/art_workbench 子目录逐文件研究记录（批次 B）

> 研究对象：`...\xdeditor-160\window\art_workbench\`（约 175 个 .lua）——**美术工作台/资源商店**（模型/动画/材质/物理/骨骼/捏人编辑器家族）。
> 除 window_manager/ 与 init.lua 外均为简录：依据头部实读 + `base.ui.component('...')`/`class(...)` 声明 grep 实证（标注行号），未实读全文的按目录归属与声明保守描述。

## 关键机制

- **主/子进程双形态**：`window_manager/res_window_app.lua:1-4` 与 `res_window_app_winui.lua:1-4` 开头都是 `local sub = include '...res_window_app_sub'; if not ProcessInfo.is_main_process then return sub end`——**子进程拿到的是 res_window_app_sub 的消息转发桩**（res_window_app_sub.lua:1-12：metatable `__index` 把任意方法调用转成 `ProcessInfo.message_manager.send_message(nil, 'ART_WORKBENCH_APP', {op=key,...})`）。
- **全局单例**：`_G.ART_WORKBENCH_APP` / `_G.ART_WORKBENCH_APP_WinUI`（window/init.lua:30-31 由 res_window_app / res_window_app_winui `.new()` 创建）；菜单「模块/美术资源库」（menu_bar.lua:2670-2691）。
- **窗口管理器**：`win_app_manager.lua` 提供 `_G.WINDOW_APP_MANAGER`（:162）与 `base.ui.create_window_app(props)`（:165-183，V2 触发编辑器也用）——详见 window-a.md 机制总结。
- **独立运行模式**：`art_workbench/init.lua:5-30`——argv 带 `-art_workbench` 时隐藏主窗口，只起美术工作台（可带预览路径直接定位资源）。
- **app 类风格**：各编辑器 app 用 `class/base.class('XxxApp')` + `gen_win_id()` 自增窗口 id（anim_editor_app.lua:11-15、skeleton_editor_app.lua:7-12、retarget_editor_app.lua:7-12、kawaii_physics_app.lua:7-12、control_rig_app.lua:7-12、modeling_win_app.lua:7-11），经 window/init.lua:32-38 实例化为 `_G.ANIM_EDITOR_APP` 等。

## 根级（5 个）

- **art_workbench/init.lua**：美术工作台独立入口（argv 门控，:1-30；`require "window"` :12 复用整个 window 引导）。依赖 `@common.base.argv`。
- **art_workbench/art_workbench_settings.lua**：渲染设置表（后处理开关/RENDER_PATH，:1-12）。纯数据。
- **art_workbench/art_workbench_ui.lua**：`art_workbench_ui` 组件（:23），工作台主 UI（ResourceStore 插件面板宿主，res_window_app.lua:35-36 佐证）。
- **art_workbench/art_workbench_ui_scene.lua**：`art_workbench_ui_scene` 组件（:11），场景预览区。
- **art_workbench/art_workbench_ui_drag.lua**：拖拽支持（按文件名简录）。

## window_manager/（8 个，详写）

- **window_manager/win_app_manager.lua**：全局窗口管理器。导出 `_G.WINDOW_APP_MANAGER = win_app_manager.new()`（:162）+ `base.ui.create_window_app`（:165-183）。`create_window(props)`（:26-47）、`close_all`（:53-57）、`handle_window`（:148）。依赖 `window.window_app`（:3）、`window_border`（:4）。**补丁相关：全编辑器窗口的总登记处。**
- **window_manager/res_window_app.lua**：美术工作台主 app 类 `res_win`（:9）；UI 模板 = ui_title_bar（:17-28）+ sce.ui.tabs_panel + art_workbench_ui（:30-40）。依赖 ui_title_bar/anim_libs.tabs/config.title_items_config/common.res_path_kit（:5-12）。子进程返回 sub 桩（:1-4）。
- **window_manager/res_window_app_winui.lua**：WinUI 版资源库 app（同结构，:9；供 menu_bar.lua:2697 的 C# ResourceLibrary 模块使用）。
- **window_manager/res_window_app_sub.lua**：子进程消息转发桩（:1-15）。
- **window_manager/window_border.lua**：`window_border` 组件（:9），窗口边框。
- **window_manager/message_box.lua**：工作台消息框（按文件名简录）。
- **window_manager/menu_config/menus.lua**：`res_menu_ui` 菜单组件（:6），资源树右键菜单。
- **window_manager/menu_config/menu_items_manager.lua**：菜单项管理（按文件名简录）。

## animation_editor/（11 个，动画编辑器）
- **anim_editor_app.lua**：动作编辑 APP（头注释 :1-3；`AnimEditorApp` :11）→ `_G.ANIM_EDITOR_APP`。依赖 window_title/anim_scene_ui/anim_editor_const/anim_editor_event（:6-9）。
- anim_scene_ui.lua（`anim_scene_ui` :28）、anim_controller_ui.lua（`anim_controller_ui` :31）、anim_info_ui.lua（`anim_info_ui` :13）、anim_title_bar.lua（`anim_title_bar` :3）、anim_track_panel.lua（`anim_track_panel` :36）、anim_tree.lua（`anim_tree` :7）、anim_tree_item.lua（`anim_tree_item` :19）、scroll_panel.lua（`scroll_panel` :5）——动画编辑器各 UI 组件。
- anim_editor_const.lua / anim_editor_event.lua：常量与事件定义（按文件名简录）。

## anim_libs/（9 个，动画库/预览）
- anim_tab.lua（`anim_libs` 组件 :36）、tabs.lua（`tabs` :4）、tab_item.lua（`tab_item` :2）、anim_info.lua（`anim_info` :8）、anim_preview_component.lua（`anim_preview_component` :14）、anim_preview_item.lua（`anim_preview_item` :23）、anim_view_component.lua（`anim_view_component` :13）——动画库标签页与预览组件。
- anim_preview_scene_ctrller.lua：`anim_preview_scene_ctrller`（`base.class(..., SCE.SceneController)` :5），预览场景控制器。
- anim_conf.lua：动画配置（按文件名简录）。

## art_material_editor/（10 个，材质编辑器）
- material_editor_ui.lua（`art_material_editor` :11）主组件；material_form.lua（`material_form_view` :361）、material_template_form.lua（`material_template_viewer` :11 / `material_template_form` :295）、material_list.lua（`material_list` :4）、material_collapse.lua（`material_collapse` :5）、material_selector.lua（`material_selector` :12）、texture_selector.lua（`texture_selector` :6）、material_popup_window.lua（`popup_window` :5）、material_popup_panel_trigger.lua（:5）——材质编辑 UI 家族。
- material_scene.lua：`material_scene_ctrller`（`class(..., SCE.SceneController)` :3），材质预览场景。

## modeling_editor/（29 个，捏人/换装编辑器）
- **modeling_win_app.lua**：捏人&换装窗口（:1；`class('change_clothes_win_app')` :2），`gen_win_id`（:7-11），构造即建窗（:13-15）。依赖 modeling_main_ui（:4）。
- init.lua：modeling 模块引导（按文件名简录）。
- modeling_main_ui.lua / modeling_scene_view.lua / view_scene_tree.lua：主 UI / 场景视图 / 场景树（按文件名简录）。
- core/：modeling_proxy.lua（编辑器代理）、meta_human_cfg.lua（metahuman 配置）、cloth_editor_kawaii_physics_anims.lua（布料物理动画数据）。
- meta_human/（6）：meta_human_ui.lua（`meta_human_ui` :17）、body_setting_ui.lua（:13 `somatotype_setting_ui` 见下）、somatotype_setting_ui.lua（`somatotype_setting_ui` :13）、body_part_setting_ui.lua（:8）、body_color_ui.lua（:13）、colortint_ui.lua（`colortint_ui` :8）。
- common/（15）：bottom_bar.lua（`modeling_editor_bottom_bar` :2）、collapse_window.lua（`collapse_window` :3）、confirm_page.lua、custom_tab.lua（`custom_tab` :7）、custom_tab_item.lua（:2）、custom_tab_title.lua、custom_window.lua、custom_list_view.lua、custom_list_item.lua、expand_bar.lua、part_collapse.lua、res_copy_helper.lua、save_window.lua（另被 res_move_tool 引用）、update_characters1.lua、res_view_ui/preview_item_ui.lua。

## physics_editor/（3）/ physics_animation_editor/（4）/ skeleton_editor/（2）/ retarget/（4）/ kawaii_physics/（2）/ control_rig/（2）
- physics_editor_app.lua：物理（碰撞）编辑窗口类（:1-8）→ `_G.PHYSICS_EDITOR_APP`；physics_editor_ui.lua（UI）；app_title_bar.lua（标题栏）。
- physics_anim_editor_app.lua：Spring Bone 编辑窗口（:1-6）→ `_G.PHYSICS_ANIM_EDITOR_APP`；physics_anim_editor_ui.lua（`physics_anim_editor_ui` :16）；ui/physics_anim_scene_ui.lua（:10）、ui/physics_anim_title_bar.lua（:3）。
- skeleton_editor_app.lua：骨骼编辑窗口（:1-7）→ `_G.SKELETON_EDITOR_APP`；skeleton_editor_ui.lua（`skeleton_editor_ui` :23）。
- retarget_editor_app.lua：重定向编辑窗口（`RetargetEditorApp` :7）→ `_G.RETARGET_EDITOR_APP`；retarget_scene_ui.lua（`retarget_scene_ui` :17）、retarget_control_panel.lua（:10）、retarget_scene_control_panel.lua（:8）。
- kawaii_physics_app.lua（:7）+ kawaii_physics_ui.lua（`kawaii_physics_ui` :24）。
- control_rig_app.lua（:7）+ control_rig_ui.lua（`control_rig_ui` :25）。

## component/（48 个，工作台通用组件库；均由 window/init.lua:13 `require 'window.art_workbench.component.ui'` 加载）
- 根级（17）：ui_title_bar.lua（`ui_title_bar` :3，工作台标题栏）、ui_tree.lua（`ui_tree_node` :13 / `ui_tree` :258）、ui_snapshot.lua（`ui_snapshot_component` :35）、ui_auto_snapshot.lua（:15）、ui_basic_info_v2.lua（`ui_basic_info_v2` :85）、ui_slot_list.lua（`slot_list_view` :4）、unfixed_content_tab.lua（:14）、common_form.lua（`brightness_input` :27 / `common_form` :131）、normal_scene_ui.lua（`normal_scene_ui` :44）、sound_scene_ui.lua（`art_workbench_sound_editor` :26）、res_good_info_view.lua（`good_info_view` :6）、scene_control_panel.lua（:8）、scene_control_utils.lua（`toggle_button` :19 / `select_button` :74 / `control3D_panel` :122）、simple_scene_control_panel.lua（:9）、scene_tree.lua（`scene_tree` :6）、scene_tree_item.lua（:5）、transform_panel.lua（`transform_panel` :4）。
- basic_info/（8）：basic_animation（:112）/ basic_colortint（:2）/ basic_light（:65）/ basic_lod（:3）/ basic_material（:3）/ basic_slot（:6）/ basic_socket（:6）/ basic_style（:233）——资源基本信息分区组件。
- ui/（6）：init.lua（组件库引导）、label_basic（:4）/ label_input_number（:6）/ label_input_vec3（:5）/ label_select（:6）。
- ui_array/（4）：array（`array_v2` :15）/ item（`item_v2` :79）/ line（`line_v2` :1）/ scene_view（`scene_view` :10）。
- ui_tree/（7）：tree_frame（:5）/ tree_framework（`ui_tree_framework` :7）/ tree_item_datas / tree_items/ 下 tree_item_bone（:28）/ tree_item_mesh（:18）/ save_as / save_as_dialog。
- tabloid/（3）：tabloid（:8）/ tabloid_left（:19）/ tabloid_top（:22）；tabloid_window/ui.lua（1，动态组件名 `'tabloid_window' .. art_id` :19）。
- particle_scene_ui/（2）：particle_scene_ui（`art_workbench_particle_editor` :95）/ scene_bar（:10）。

## common/（9 个，资源操作）
- import_res.lua（导入资源，`user_import_mechanism`，menu_bar.lua:2596 引用）、import_history_operation.lua、label_operation.lua、lod_set.lua、publish_good.lua（`hyperlink_label` :6，发布商品）、ref_file.lua、res_path_kit.lua（资源路径工具，被全库广泛引用）、uncategorized_res.lua、update_res.lua。

## config/（4 个）
- title_items_config.lua（工作台标题栏页签配置，res_window_app.lua:11 引用）、default_sockets_config.lua、eqpt_affix.lua（装备词缀）、texture_import_error_code.lua。

## main_ui/（8 个，商店主界面）
- batch_upload_icons_ui.lua、category_and_local_tree_view_ui.lua、connect_interrupted_tips.lua、effect_ui.lua、label_preview.lua、label_preview_panel.lua、label_ui.lua、upload_good_ui.lua（按文件名简录，商店上传/标签/分类树 UI）。

## res_items/（8 个，资源条目）
- category_tree_generater.lua、explore_tree_generater.lua、res_tree_generater.lua（三棵树生成器）、project_res_manager.lua、res_data_manager.lua、res_searcher.lua、res_sort.lua、res_utils.lua（被 menus.lua:3 引用）。

## aigc/（3 个，AI 生成头像）
- ai_avatar_component.lua（`ai_avatar_component` :7）、ai_avatar_info_panel.lua（同名组件 :7）、ai_avatar_item.lua（`ai_avatar_item` :7）。

## colortint_editor/（1）
- colortint_editor_ui.lua（`colortint_editor` :8）。

## lod_editor/（2）
- lod_editor_ui.lua（`lod_editor` :4）、lod_form.lua（`lod_form` :1）。

## sub_dialog_windows/（2）
- add_anim_window_config.lua、save_change_cloth_window_config.lua（子对话框配置）。

## tmp_tool/（1）
- rebuild_ao_texture_material.lua（AO 贴图材质重建工具，argv `deal_ao` 门控，art_workbench/init.lua:2-4）。

补丁相关（整批）：art_workbench 各 app 均为 class 单例 + gen_win_id 多窗口模式，窗口创建统一走 WindowApp；**`res_window_app*.lua:1-4` 的 ProcessInfo 分支是主/子进程代码路径分界点**，补丁若涉及子进程需注意其只拿到消息桩。
