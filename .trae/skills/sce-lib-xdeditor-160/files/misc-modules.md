# xdeditor-160 / 批次B 逐文件研究记录（http_requests/ + ini/ + map_generator/ + map_starter/ + profiler/ + project_manager/ + ref/ + scene_test_enter_point/ + sub_process_enter_point/ + temp/ + test/ + texture_merger/ + texture_viewer/ + third-party/ + upload_map/ + utils/）

> 研究对象：`D:\sce_online\Res\maps\bgd_glzy\.editor_src_mirror\xdeditor-160\`（明文镜像）。
> 本批共 71 个 .lua：http_requests/ 3、ini/ 22、map_generator/ 1、map_starter/ 1、profiler/ 3、project_manager/ 2、ref/ 1、scene_test_enter_point/ 3、sub_process_enter_point/ 7、temp/ 1、test/ 13、texture_merger/ 1、texture_viewer/ 1、third-party/ 4、upload_map/ 2、utils/ 6。
> 全部结论来自真实读取，关键结论标注 相对路径:行号。project_manager/ 与 utils/ 详写，其余简写。

---

# http_requests/

## http_requests/goods.lua
- 用途：资源商店/商品后端 HTTP 接口封装（goodslabel 服务，9004 端口；publisher 服务 9000）。
- 导出：大表（:1052-1106）：统一入口 `post(params, cb)`（:1054）、`query(params, download, func)`（:1063）、`apply_token`、`analysis_map_name`（:1074），细接口 `query_all_goods/query_category_good_info/query_path_good_info/query_maps_good_info`（:1079-1082）、`post_map_good_info/post_map_good_category_label_info/post_map_good_image/post_map_good_module_group`（:1084-1087）、`get_my_map/get_my_res/get_my_characters1/get_my_obj/get_my_map_list`（:1089-1093）、`add_new_category_or_label/delete_map_good_info/delete_map_category_or_label_info`（:1095-1097）、`is_user_res/query_map_info/query_map_good_review/query_valid_map_good_by_category`（:1099-1105）。
- 依赖：`@common.base`（:18）、`@common.base.co`（:19）、`@common.base.argv`（:22）、`@base.base.account`（:23，@ 跨库 client_base）、`window.art_workbench.common.res_path_kit`（:25）、`http_requests.lib_node_mapping`（:26）、`project_manager`（:31）。
- 补丁相关：**写全局 `sce.goods_label_server_address()`**（:37-49）；模块加载即建两个 ResSearcher 实例扫 `Res` 与 `Update/<IP>/Res`（:27-30）——require 本模块有扫描副作用；`check_good_valid` 里 `do return true end`（:53）短路了有效性校验；`_G.editor_resource_dict()`（main.lua:131 定义）在 :58 被消费。TIME_OUT_VALUE = 5000（:34）。

## http_requests/lib_node_mapping.lua
- 用途：本地 Res 目录扫描器：把资源文件按包名/类型映射成节点树（ResLevel/ResSearcher 两个 class），含 md5→36 进制 hash 工具。
- 导出：`return ResSearcher`（:396）；`ResSearcher.new(ResRoot)`，方法 `is_parent_package/is_file_package/is_ignore_name/is_invalid_name/is_invalid_path_part/collect_levels(name, base_path, root_path)/collect_level(res_path)`（:196-307）；内部 `ResLevel` 类（:135-186）：`build_package_name/normalize/to_dict`。
- 依赖：无 require（自带 base.class 兜底 :1-9）。PackageType 常量表（:15-22：prefab/otf/ttf/ani/ogg/static）。
- 补丁相关：**定义了全局函数 `init_hash_mac`/`hash_to36`**（:27、:43，未 local 化，污染 _G）；`base.class` 兜底实现（:5-9）说明本文件可在无 @common.class 环境独立跑。

## http_requests/lib_node_mapping_test.lua
- 用途：lib_node_mapping 的独立测试脚本（test_res_search，对照 walk_absolute_dir 验证 collect_levels）。
- 导出：无。
- 依赖：`require 'global'/'utils'/'config'`（:2-4）、`http_requests.lib_node_mapping`（:6）。
- 补丁相关：无（测试代码，硬编码 `D:/NE_for_lib_node_mapping/Res` :7）。

---

# ini/（数据编辑器 ini 读写核心）

> 数据流（ini/manager.lua:1-10 注释）：`file <--> ini_string <--> ini_ast <--> ini_table <--> ini_data <--> ui`；特性：保留注释、保留字段顺序、按配置顺序存新字段、删与默认表相同字段、继承读写。

## ini/manager.lua
- 用途：ini 子系统门面：load_map（建 current_map file_manager + object_tree.preprocess）、get_map、save_changes。
- 导出：`return { load_map, get_map, save_changes, object_tree, ini_data, common }`（:35-46）。
- 依赖：`ini.common`（:11）、`ini.enum`（:12）、`ini.file_manager`（:13）、`ini.ini_data`（:14）、`ini.object_tree.object_tree`（:15）。
- 补丁相关：`load_map(dir)`（:17-29）里 `file_manager.remove('current_map')` + 重设 file_manager 元表指向新地图（:22-24）——**换图时 ini 子系统靠元表切换数据源**。

## ini/common.lua
- 用途：ini 子系统公共工具：path 对象（重载 `+`/`..`/`<<` 拼路径 :17-34）、trim/split/deep_equal/tree_builder。
- 导出：`return { map_dir=nil, path, trim, split, deep_equal, tree_builder }`（:204-211）。
- 依赖：无。
- 补丁相关：`map_dir` 字段由 manager.load_map 写入（manager.lua:21），是 ini 子系统的全局当前地图路径。

## ini/enum.lua
- 用途：枚举字段处理：读 config/ini_table/enum.lua 的枚举 AST，生成 select 选项与「枚举值↔枚举名」互转。
- 导出：`return { set_to_map, encode_ast, encode_data }`（:161-165）。
- 依赖：`ini.common`（:3）、`ini.file_manager`（:4）。
- 补丁相关：`set_enum_optinos_map`（:20-29）直接读 ast 内部结构（sections/name.value）。

## ini/file_manager.lua
- 用途：.ini/.iniconfig 文件管理池：按 alias 管理多个 file_manager（current_map 等），负责加载/保存、表名重复检查、循环继承检查。
- 导出：`return setmetatable(file_manager_pool, { __index = file_manager_pool.get('current_map') })`（:600）——**默认直接代理 current_map 的方法**；池接口 `get(alias, root_path)`（:578）、`remove(alias)`（:588）。
- 依赖：`ini.ast.ast`（:7）、`ini.common`（:8）。
- 补丁相关：**模块加载即 `get('current_map')`（:600）**；错误聚合输出 `base.loop(2000, ...)`（:29-37）每 2 秒批量 log.error；单个 manager 方法含 add_iniconfig/remove_iniconfig/exist_iniconfig/exist_section_name/get_file_info/get_recorded_file_info/get_can_inherit_section_name_map/read_all_data（:560-573）。

## ini/ini_data.lua
- 用途：按 config_map 的 UI 配置读写 ini 数据（load_config/read_ini_data/save_ini_data/read_ini_data_base/get_ini_base_name）。
- 导出：`return default_ini_data`（:211）；方法表 :187-196；`default_ini_data.get(name)` 按 alias 取独立实例（:202-209）。
- 依赖：`config.ini_table.config_map`（:2）、`ini.common`（:3）、`ini.enum`（:4）、`ini.file_manager`（:5）。
- 补丁相关：`load_config(config_type)`（:15-40）把 table_key 拆成 keys 并计算 configs_order_map（字段顺序来源）。

## ini/ast/ast.lua
- 用途：ini_ast 类（`---@class ini_ast` :150）：封装 AST 的全部操作。
- 导出：`return Ast`（:201，构造函数 `Ast(ini_string, info)` :197-199）；方法：serialize/read_data/load_data/get_order/remove_section/clear_sections/remove_parameters/find_section/sort_section/get_parameter_list/get_section_list/change_value_type/get_section_base/set_section_base（:153-194）。
- 依赖：`ini.ast.parse/serialize/read_data/load_data/get_order`（:5-9）。
- 补丁相关：这是改 ini 数据最干净的入口；`set_section_base` 是继承机制（:192-194）。

## ini/ast/parse.lua
- 用途：ini_string → ini_ast 解析器（token 位掩码 T_equal/T_comma/T_colon/T_lcurly… :22-25）。
- 导出：`return parse`（:517）。
- 依赖：`ini.ast.escape`（:5）。
- 补丁相关：无。

## ini/ast/serialize.lua
- 用途：ini_ast → ini_string 序列化（保留注释 serialize_comments :24）。
- 导出：`return serialize`（:189）。
- 依赖：`ini.ast.escape`（:3）。
- 补丁相关：无。

## ini/ast/read_data.lua
- 用途：从 ini_ast 读数据为 Lua table（identifier 经 identifier_replace_map 替换 :3-16）。
- 导出：`return read_data`（:74）。
- 依赖：无。
- 补丁相关：无。

## ini/ast/load_data.lua
- 用途：向 ini_ast 写数据（按 config_order 排序写入 :7；含 \n 字符串自动转 long_string :24-25）。
- 导出：`return load_data`（:149）。
- 依赖：无。
- 补丁相关：无。

## ini/ast/get_order.lua
- 用途：取 ini_ast 中字段顺序（按 paths 逐层下钻 :3-19）。
- 导出：`return get_order`（:68）。
- 依赖：无。补丁相关：无。

## ini/ast/escape.lua
- 用途：字符串转义/反转义（escape_map :5-16，支持 \ddd 数字转义 :21-25）。
- 导出：`return { escape, add_escape }`（:52）。
- 依赖：无。补丁相关：无。

## ini/object_tree/object_tree.lua
- 用途：单位树门面：聚合 generater/preprocess/node_type 与四套事件管理器。
- 导出：`return { generater_object_tree(map_path), preprocess, node_type, code_node_type, object_event_manager, trigger_event_manager, scene_event_manager, backend_script_manager }`（:11-30）。
- 依赖：`ini.object_tree.generater/node_type/preprocess/event_manager/register_object_tree_event`（:1-8）。
- 补丁相关：**事件管理器按名字取**：`event_manager.get('object_tree'/'trigger_tree'/'scene_tree'/'backend_script_manager')`（:4-7）；require 本模块即执行 register_object_tree_event（:8，注册全部节点事件，副作用大）。

## ini/object_tree/generater.lua
- 用途：从地图 ini 文件生成单位树结构（generate_root），含 add_child/remove_child 与右键菜单桥接。
- 导出：`return generater`（:427）。
- 依赖：`ini.common/file_manager`、`ui.resource_tree_tool_ui`、`ini.object_tree.node_type/event_manager`（:2-7）。
- 补丁相关：`menu_click`（:9-12）经 event_manager:menu_click 拿菜单数据后 `ui.show_menu`。

## ini/object_tree/node_type.lua
- 用途：节点类型定义：icon_type 表（unit/model/skill/code/variable 等图标映射 :3-20）与 code_node_type。
- 导出：`return { type=..., code_node_type=... }`（:165）。
- 依赖：无。补丁相关：无。

## ini/object_tree/copyer.lua
- 用途：单位树节点复制/粘贴逻辑（含 print_r 调试函数 :9-20）。
- 导出：`return mt`（:557）。
- 依赖：`ini.common/file_manager/ini_data`、`ui.resource_tree_tool_ui`、`node_type`、`generater`（:1-8）。
- 补丁相关：无。

## ini/object_tree/operator.lua
- 用途：单位树节点操作集（增删改、与触发器 AST 联动）。
- 导出：大表 `return { ... }`（:1802）。
- 依赖：`ini.common/ini_data/file_manager`、`generater/node_type/copyer/preprocess`、`ui.resource_tree_tool_ui`（:1-9）、`trigger.trigger_manager`、`trigger.type_infer_basic`、`trigger.lua-parser.parser/utils`、`trigger.lua-parser.change_dealer`、`trigger.lua-generator`（:12-20，**与触发器子系统深度耦合**）。
- 补丁相关：无直接 hook；是「数编改字段 → 触发器代码联动改写」的枢纽。

## ini/object_tree/outer_object_tree.lua
- 用途：资源库（sample_units）外部单位树生成器，namespace = 'sample_units'（:3）。
- 导出：outer_generater 表（含 sample_units_dir）。
- 依赖：`ini.common`、`ini.file_manager'.get('sample_units')`（:4）、`node_type`、`@common.base.path`（:9）、`@common.base.argv`（:14）。
- 补丁相关：`use_local_res` argv 决定 root_paths 顺序（:15-25 注释区可见策略）。

## ini/object_tree/preprocess.lua
- 用途：ini 预处理：父子单位关系（__sub_units :8）与 buff 从属关系补全。
- 导出：`return process`（:108）。
- 依赖：`ini.common`、`ini.file_manager`（:5-6，file_manager 延迟赋值 :6）。
- 补丁相关：manager.load_map 末尾调用（manager.lua:28）。

## ini/object_tree/register_object_tree_event.lua
- 用途：注册各节点类型的点击/添加/右键事件（单位树/触发树/场景树/后端脚本树/信息列表/异常列表）。
- 导出：无（执行型）。
- 依赖：`plugin.model_editor.components.scene_operation`（:2）、`ini.common/file_manager`、`ui.resource_tree_tool_ui`、`node_type`、`event_manager`（五个 get）、`operator`（:21）、`ImportSCEContext()`（:22-25）。
- 补丁相关：object_tree.lua:8 require 时执行；**加自定义树节点事件的官方入口是 `event_manager.get('<树名>'):get_on_click(node_type)` 系列**。

## ini/object_tree/event_manager.lua
- 用途：树节点事件管理器类：on_click/on_down/on_add_click/on_right_click 四类回调表 + menu_click 菜单分组拼装。
- 导出：`return managers`（:191，`managers:get(name)` :178-189 按名懒建，**调用即打日志 `register_manager_get`** :184）。
- 依赖：无。
- 补丁相关：`menu_click`（:160-173）按组拼菜单并支持 check_enable 置灰。

## ini/object_tree/sample_section_box_ui.lua
- 用途：单位库弹框 UI（从 Res sample_units 选表，带搜索树）。
- 导出：`return { ... }`（:214）。
- 依赖：`@appui`（:1）、`ui.components.tree_with_search`（:2）、`@common.base.path`（:4）、`outer_object_tree`（:5）、`ini_data/file_manager`（:6-7）。
- 补丁相关：`root_path = path('Update') / _G.IP`（:9）——**依赖 main.lua 设置的 `_G.IP` 全局**。

---

# map_generator/

## map_generator/init.lua
- 用途：**命令行生成/发布流程**（generate_cmd 分支）：生成地图、发布地图、发布依赖库（upload_lib/upload_lib_abs）。
- 导出：`return { generate_map, upload_map, upload_lib, upload_lib_abs }`（:326-331）。
- 依赖：`require 'global'/'utils'`、`include 'config'`、`require 'console'`（:1-4，自带三件套）、`@common.base.co`（:5）、`trigger.trigger_manager`（:9）、`upload_map.upload_lib`（:45、:189）、`ui.utils`（:77、:284）。
- 补丁相关：`_generate_map` = `EDITOR.load_map(..., force_use_editor_api=true)` + `EDITOR.save_map(false, '生成地图', true)`（:13-20）；`upload_lib_abs` 支持 `lib_path,lib_name,api_version` 参数串（:191-197），`server_common` 会额外生成 `server_common2` 加密变体（:257-267，encrypt=2）；`lib_encrypt_map`/`lib_need_pak_map` 取自 trigger_manager（:11-12）。TRIGGER_EDITOR_V2 库发布前删 node_modules（:87-89）。

---

# map_starter/

## map_starter/init.lua
- 用途：**generate_and_debug_map 分支入口**：登录后加载当前地图 → 生成 lua → 另存到调试目录 → 拉起远端/本地调试游戏，结束 `os.exit(0)`。
- 导出：无（执行型，尾部分支 :171-192）。
- 依赖：`trigger_editor_v2.lua.trigger_manager.trigger_manager`（:1）、`@common.base.account`（:2）、`project_manager`（:3）、`@common.base.co`（:4）、`plugin.tile_editor.ui_resolution_content`（:5）、`@base.update.core.api_version_config`（:6，@ 跨库 client_base）。
- 补丁相关：`query_assign_host()` 向 `http://<_G.IP>:9007/api/v1/assign_host` 申请调试 host（:13-43，带 api_version）；`_G.__fortest_still_use_local_host`（:111）是本地调试后门；`clear_folder` 保留 `.sce_workspace.code-workspace`（:73）与 `.vscode`（:86）；`debug_save_as` 内 `co.call(DebugManager.debug_game, ...)`（:148-156）真正拉起调试；主流程从 main.lua:715（登录回调 `require 'map_starter'`）进入。

---

# profiler/

## profiler/profiler_ui.lua
- 用途：性能分析器主 UI（ProfilerUI 类：线程数据按钮、debug_exec 勾选、帧耗时折线 handle='__profiler_line__' :7，max_cost=66ms 即 15fps 标线 :8）。
- 导出：`return ProfilerUI`（:1087，class('ProfilerUI') :5）。
- 依赖：`config.ui.style`（:2）、`@appui`（:3）、`ui.components.window_title`（:4）、`profiler.profiler_tree`（:6）。
- 补丁相关：由 window/profiler_app.lua 或 sub_process_enter_point/main.lua:13-20（`-profiler` argv 分支建 `_G.PROFILER_APP`）使用。

## profiler/profiler_tree.lua
- 用途：profiler 调用树 UI 组件（base.ui.component('tree_content') 自定义组件 :3-4）。
- 导出：`return tree`（:677）。
- 依赖：base.ui.component。
- 补丁相关：组件写法样板（`tree_content:define()` 内 props + template，注释 :1 提示用法）。

## profiler/profiler_time_ui.lua
- 用途：性能耗时条 UI（ProfilerTimeUI 类，面板 id '__profiler_time_strip__' :27）。
- 导出：`return ProfilerTimeUI`（:42）。
- 依赖：`config.ui.style`（:2）、`@appui`（:3）、`ui.components.window_title`（:4）。
- 补丁相关：由 window/profiler_time_app.lua 包装成独立窗口 app。

---

# project_manager/（详写）

## project_manager/init.lua
- 用途：入口桩：`return require 'project_manager.project_manager'`（:1）。
- 补丁相关：全库统一 `require 'project_manager'`（main.lua:321、:480 等）实际拿到下面这个表。

## project_manager/project_manager.lua
- 用途：**项目（地图包）设置与项目管理 API**：对 `SCE.GetProjectSettings()` 的 Lua 封装 + publisher 服务 HTTP 接口。
- 导出：`return project_manager`（:546）。全部方法（行号）：
  - 文件/设置：`save_project_file(map_path)`（:57，`SCE.GetProjectSettings():save`）、**`load_project_file(map_path)`**（:61-64，load 后 `EDITOR.event_notify(EVENT.update_project_settings)` :63——项目设置变更的统一通知点）、`get_editor_project_settings/set_editor_project_settings`（:526、:537）。
  - 项目元信息：`get_project_name(just_project_name)`（:119-133，MapSettings 空时回退文件夹名）、`get_template_name`（:135）、`set_project_name`（:194）、`set_template_name`（:202）、`get_project_is_lib/set_project_is_lib`（:210、:215）、`get_project_accessable/set_project_accessable`（:223、:228）。
  - 编辑器模式：`get_trigger_editor_mode/set_trigger_editor_mode`（:145、:150，默认 SCE.TRIGGER_EDITOR_V1 :147）、`get_mechanism_mode/set_mechanism_mode`（:155、:160）。
  - 路径/调试：`get_test_map_path/set_test_map_path`（:178、:187）、**`get_project_map_dirs(with_editor)`（:476）/ `get_project_map_files()`（:491）/ `get_debug_copy_dirs()`（:503）/ `get_debug_copy_files()`（:507）**——发布/调试拷贝的目录与文件白名单（utils/event.lua:745-746、map_starter/init.lua:138-139 消费）。
  - 网络：`get_client_id_url()`（:66-74，PD 环境返回 main.production.spark.xd.com，否则 `_G.IP`）、`get_client_id()`（:76-116，GET `:9090/api/v1/create-client?map_name=`）、**`create_project_name(file_name, callback, package_type, path, source)`**（:245-282，POST publisher:9000 `/api/map/create-project`，package_type 注释 :264「1地图 4依赖库 8大厅」）、`create_equip_project(project_id, cb)`（:285-318，走 lobby.apply_token）、`set_project_package_score_name`（:332）、`get_project_package_score_name`（:387）、`get_enable_set_score_name/set_enable_set_score_name`（:434、:439）、`check_project_use_score`（:447）、`save_score_name`（:510）。
- 依赖：`@common.base.lobby`（:3）、`@common.base.util`（:4）、`ini.ast.ast`（:5）、`@common.base.co`（:6）、`@common.json`（:7）、`@common.base.account`（:8）、`@common.base.ip`（:67，函数内延迟 require）。
- 补丁相关：**全部设置读写都过 `SCE.GetProjectSettings():get_module_settings('MapSettings')`**（:120、:136、:146…）——C++ 侧 ProjectSettings 是唯一数据源，Lua 侧无缓存（顶部 :14-56 大段注释掉的 package_files 缓存方案）；`load_project_file` 由 main.lua 的 EVENT.load_map 处理器调用（main.lua:329）。HTTP 调用统一模式：`coroutine.call(account.http_request_with_token, sce.httplib.create(), {...})`（:83、:257、:345）。

---

# ref/

## ref/init.lua
- 用途：**资源引用（ref）计算**：保存地图时生成 objref.txt/fontref.txt/libsref.txt/filter.json/editor_objref.txt 及各场景 ref。
- 导出：无（执行型，靠注册事件生效）。
- 依赖：`plugin.tile_editor.select_list_view.scene_list_manager`（:1）、`trigger.trigger_manager`（:2）、`trigger_editor_v2.lua.trigger_manager.trigger_manager`（:3）、`plugin.obj_editor_v2.utility`（:4）、`plugin.localization_manager`（:5）、`project_manager.project_manager`（:6）、`plugin.obj_editor_ui.ui.tools.model_animation_tools`（:7）、`plugin.obj_editor_v2.const`（:10）。
- 补丁相关：**`EDITOR.event_register(EVENT.save_map_progress_obj_ref, ...)`（:248-279）**——存图管线的 ref 计算环节（utils/event.lua:608 以 save_plugin 调起，支持 promise 异步）；`generate_obj_ref`（:177-221）/ `generate_editor_ref`（:223-241）/ `generate_all_ref`（:243-245）；objv1 argv 切换 obj_editor/obj_editor_v2（:180-184、:226-230）；`libs.json` 间接引用 BFS（:87-124）。

---

# scene_test_enter_point/

## scene_test_enter_point/main.lua
- 用途：`-scene_test` 分支入口：建测试场景（SceneBuilder 构建 map.acmap）、相机（WASD 或读 camera_info.json 的 Moba 相机），注册 'TestScene'。
- 导出：无。
- 依赖：`@common.base.argv`（:5）、`@common.device_settings`（:10）、`SCE.GetSceneManager()`（:7）。
- 补丁相关：**`_G.sample_scene_controller = controller`**（:72）；`class('TestSceneControler', SCE.SceneController)`（:8）是 Lua 继承 C++ 场景控制器的样板；三层 `base.next` 延迟初始化视口/阴影（:89-103）。由 main.lua:27 require。

## scene_test_enter_point/height_map.lua
- 用途：地编场景测试：HeightMapSceneControler + TileEditorUIDebugComponent（LuaUIDebugComponent 子类 :11）+ 调试按钮。
- 导出：无。
- 依赖：`@appui`（:5）、`SCE.GetSceneManager()/GetPluginsManager()/GetEventManager()`（:7-9）。
- 补丁相关：`class('TileEditorUIDebugComponent', SCE.LuaUIDebugComponent)`（:11）是 Lua 挂 UI 调试组件的样板。

## scene_test_enter_point/task_pipline_unit_test.lua
- 用途：任务管线（TaskPipline）单元测试：注册多优先级任务并验证执行序。
- 导出：无。
- 依赖：`SCE.GetTaskPiplineManager()`（:6）。
- 补丁相关：`class('LoadMapContext', SCE.TaskPiplineContext)`（:7）、`pipline:register_task({run_type = SCE.RunInPiplineCallingThread, priority = 1}, fn, 'Step1')`（:18-27）是任务管线 API 样板；全局 MAP_TASK_PRIORITY（global/global.lua:232）与本管线配套。

---

# sub_process_enter_point/

## sub_process_enter_point/main.lua
- 用途：`-sub_process` 分支入口（main.lua:42 require）：按 argv 分派 progress_bar / message_box / profiler 子进程 UI。
- 导出：无。
- 依赖：`@common.base`（:1）、`@common.base.argv`（:2）、`sub_process_enter_point.progress_bar`（:3）、`sub_process_enter_point.message_box`（:4）；`profilernew` 分支 `SCE.Common.create_csharp_module('ProfilerClient')`（:10-12）；`profiler` 分支 `window.profiler_app`（:14）建 `_G.PROFILER_APP`（:19-20）。
- 补丁相关：**注册 `shortcutMgr.RELOAD → EDITOR.event_notify('reload') + app.reload()`（:34-38）**；`inner` argv 下 F1 键同样触发 reload（:40-48）——子进程热重载入口。

## sub_process_enter_point/init_process_info.lua
- 用途：注册 `_G.ProcessInfo` 全局（主/子进程判断 + 子进程管理器 + 消息管理器）。
- 导出：无。
- 依赖：`sub_process_enter_point.sub_project_message_manager`（:9、:16）、`sub_process_enter_point.events`（:19）。
- 补丁相关：**主流程在 main.lua:124 include 本文件**（早于登录）；有 `SCE.GetSubprocessManager` 时 `is_main_process` 取真实值（:2-10），否则默认 true（:12-17）；utils/event.lua:299 用 `ProcessInfo.process_manager:open_project(map_path)` 实现「主进程已开图时再开图走子进程」。

## sub_process_enter_point/events.lua
- 用途：子进程消息注册集中地：目前仅子进程侧注册 'resource_dirty' → `SCE.ReloadResourcesByPrefix(res_dir)`（:4-7）。
- 导出：无。
- 依赖：无（用 `_G.ProcessInfo`）。
- 补丁相关：主进程 resource_dirty 广播 → 子进程热重载资源的通道。

## sub_process_enter_point/sub_project_message_manager.lua
- 用途：主/子进程消息总线：send_message/register/unregister，参数 json 编码且**自动带上自己地图路径**（:6-12）。
- 导出：`return { send_message, decode_args, register, has_register?, unregister... }`（:47-50）。
- 依赖：`SCE.GetSubprocessManager()`（:2）、`base.game:broadcast('process_message', ...)`（:41-45，消息接收统一入口）。
- 补丁相关：`base.game:broadcast('process_message')` 是跨进程消息的事件名，可被补丁监听/伪造。

## sub_process_enter_point/progress_bar.lua
- 用途：子进程进度条窗口 UI（appui 主题色面板 + tips 轮播）。
- 导出：`return { init, ... }`（:418）。
- 依赖：`@appui`（:2，theme.get_current_theme() :3）。
- 补丁相关：无。

## sub_process_enter_point/message_box.lua
- 用途：子进程消息框 UI（`SCE:GetEMessageBox()` :3）。
- 导出：`return { init, ... }`（:219）。
- 依赖：`@appui`（:2）。
- 补丁相关：无。

## sub_process_enter_point/test.lua
- 用途：子进程机制手工测试（B/N 键 open_project、M 键发消息、test/test_receive 消息互发）。
- 导出：无。
- 依赖：`SCE.GetSubprocessManager()`（:2）、`ProcessInfo.message_manager`（:11）。
- 补丁相关：在 init_process_info.lua:20 被注释掉，不加载；硬编码 `C:/NE/Res/maps/...` 路径。

---

# temp/

## temp/scale_deco_and_area.lua
- 用途：一次性工具脚本：批量缩放地图场景装饰物/区域坐标（直接文本替换 map.scene_items 里 Position3D 的 x/y/z :12-30）。
- 导出：无。
- 依赖：`SCE.GetEventManager()`（:2）。
- 补丁相关：注释 :4「File From bat/其他工具/change_scale」——运维脚本沉淀；无人 require（全库无引用即死代码，需自行确认）。

---

# test/（13 个测试/草稿文件，均无生产引用价值）

## test/draft.lua
- 用途：触发器函数名草稿（纯文本行 func_play_animation 等 :1-3，非合法 Lua 语句集合）。
- 导出：无。依赖：无。补丁相关：无。

## test/level.lua
- 用途：游戏关卡脚本草稿（几乎全部注释掉，`require 'game.地图配置'` 等 :1-18）。
- 导出：无。依赖：无（注释内引用不算）。补丁相关：无。

## test/level_full.lua
- 用途：level.lua 的完整版草稿（开头即 `require 'game.地图配置'` :2）。
- 导出：无。依赖：无。补丁相关：无。

## test/level2.lua / level_3.lua / level_4.lua / level_5.lua
- 用途：声明/语法测试草稿（大量注释掉的测试声明 :1-6）。
- 导出：无。依赖：无。补丁相关：无。

## test/level_enum.lua / level_struct.lua / level_typedef.lua
- 用途：LuaPlus 类型标注语法试验（`local p : table<a:number, b:string>` 等，level_struct.lua:1）与 enum 定义草稿（level_enum.lua:1-12 注释）。
- 导出：无。依赖：无。补丁相关：无。

## test/scorearchive.lua
- 用途：积分存档接口测试（lobby 登录后 sce.s.get_commit/score_addi/commit :11-13）。
- 导出：无。
- 依赖：`@common.base.lobby`（:1）。
- 补丁相关：main.lua:960 有注释掉的 `-- require('test.scorearchive')`；sce.s.* 积分服务调用样板。

## test/test_exp.lua
- 用途：imgui 封装实验（自建 imgui 代理表包 ui.imgui_begin_view 等 :7-14）。
- 导出：无。
- 依赖：`@common.base`（:1）、**`require '@xdeditor.global'/'@xdeditor.utils'/'@xdeditor.config'/'@xdeditor.console'`（:2-5）**——@ 前缀跨库自引用的写法实例（xdeditor 包名自指）。
- 补丁相关：无。

## test/test_text.lua
- 用途：游戏内文本/区域测试（collectgarbage 调参 :1-2、`require 'area_save'/'global_protect'/'game'` :9-15）。
- 导出：无。
- 依赖：`@common.base`（:5）。
- 补丁相关：定义全局 `stop_ui/present`（:3、:6-8，污染 _G）。

---

# texture_merger/

## texture_merger/texture_merger_ui.lua
- 用途：贴图合并工具 UI（TextureMergerUI 类，纹理选择行 texture_row :20）。
- 导出：`return TextureMergerUI`（:194，class :4）。
- 依赖：`config.ui.style`（:2）、`@appui`（:3）、`ui.components.window_title`（:5）。
- 补丁相关：由 window/texture_merger_app.lua 包装为窗口 app。

---

# texture_viewer/

## texture_viewer/texture_viewer_ui.lua
- 用途：贴图查看器 UI（TextureViewerUI 类，sRGB/RGBA 通道开关 :24-28）。
- 导出：`return TextureViewerUI`（:452，class :4）。
- 依赖：`config.ui.style`（:2）、`@appui`（:3）、`ui.components.window_title`（:5）。
- 补丁相关：由 window/texture_viewer_app.lua 包装为窗口 app。

---

# third-party/

## third-party/lua-pinyin/init.lua
- 用途：汉字转拼音库（带声调韵母表 phoneticTable :29-40、split 工具 :5-27）。
- 导出：拼音转换函数表。
- 依赖：`third-party.lua-pinyin.data.hanzi`（:1）。
- 补丁相关：被 utils/utils.lua 的 concat_pinyin（EDITOR.utils.concat_pinyin，utils.lua:1175）使用——拼音搜索数据源。

## third-party/lua-pinyin/data/hanzi.lua
- 用途：单字拼音字典（`dict["3400"]="qiū"` 按 Unicode 码位 hex 索引 :1-5）。
- 导出：dict 表。依赖：无。补丁相关：无。

## third-party/lua-pinyin/data/dict-zi-web.lua / phrases-dict.lua
- 用途：补充单字字典 / 词组拼音字典（纯数据）。
- 导出：数据表。依赖：无。补丁相关：无。

---

# upload_map/

## upload_map/upload_map_view.lua
- 用途：**发布地图窗口与上传流程**（含 30 天前 SCECheckpoint/publish 清理 :43-60、ref 检查、上传进度）。
- 导出：`return { open_window, upload_msg_box, upload_target_map, upload_target_map_ref, check_ref }`（:992-998）。
- 依赖：`config.localizatioin.localization`（:7）、`config.localizatioin.ui`（:8）、`@common.base.util`（:9）、`ini.ast.ast`（:10）、`ui.view.autosave_config_view`（:11）、`@common.base.lobby`（:12）、`@appui`（:14）、`@base.update.core.local_api_pak_version`（:15，@ 跨库）、**`@common.upload`（:16-17，upload_map/upload_ref 真身上传逻辑在 script 库）**、`project_manager`（:18）、`plugin.tile_editor.attribute_panel.terrain_attribute_panel`（:19-21）、`@base.update.core.api_version_config`（:23）、`@common.base.json_load`（:34）、`@base.base.path`（:36）。
- 补丁相关：utils/event.lua:659 `require 'upload_map.upload_map_view'`（顶层）+ :783-784 调 `upload_target_map`/`upload_target_map_ref`；`root_path = path(io.get_root_dir())`（:37）。

## upload_map/upload_lib.lua
- 用途：发布依赖库流程（upload_lib/upload_lib_immediately，含发布结果浮层 upload_success_view :19-58）。
- 导出：`return { upload_lib, upload_lib_immediately, upload_map }`（:387-391）。
- 依赖：`config.localizatioin.localization`（:1）、`@common.base.lobby`（:2）、`ui.view.upload_option_view`（:3）、`@common.base.co`（:4）、`@common.upload`（:7）、`@base.update.core.local_api_pak_version`（:9）、`@base.update.core.api_version_config`（:10）、`@base.base.path`（:12）。
- 补丁相关：map_generator/init.lua:45、:189 消费 upload_lib_immediately；`api_pak_version_manager` 读写 api 包版本。

---

# utils/（详写）

## utils/init.lua
- 用途：utils 入口五连：`include 'utils.utils'`（:1）→ `include 'utils.event'`（:2）→ `include 'utils.math'`（:3）→ `include 'utils.shortcut'`（:4）→ `include 'utils.map_download_refs'`（:5）。
- 导出：无。
- 依赖：见各文件。
- 补丁相关：**main.lua 三件套之一（main.lua:23/38/120）**。顺序有依赖：utils.lua 先建 EDITOR.utils，event.lua 再用它（EDITOR.utils.get_update_progress_func_clear 等）。

## utils/utils.lua
- 用途：EDITOR.utils 工具函数集（路径/表/库解析/进度条/拼音/HTTP IP/文档链接）。
- 导出：**`EDITOR.utils = { ... }`（:1125-1187）并 `return EDITOR.utils`（:1210）**。函数分组（定义行号 → 挂表行号）：
  - 表/字符串：print_table（:5）、table_tostring（:45）、split（:83）、trim（:92）、deep_copy（:96，支持 ignore_list）、table_equal（:199）、find_all、get_pattern、get_transferred_string、get_string_length。
  - 路径：image_path（:114，`@xdeditor/ui/images/`）、get_file_name_with_extension（:118）、get_file_name（:127）、get_file_extension（:136）、get_file_dir（:143）、add_tailing_slash（:148）、remove_tailing_slash（:156）、get_last_file_dir（:165）、get_relative_res_path（:180，按 '/Res/' 切）、trans_local_res_path（:194）、get_package_path。
  - 依赖库：has_inner_lib、set_local_libs、get_lib_path、get_lib_version_fix、get_map_templates_dir_version、get_lib_zip、get_librarys、**no_map_librarys = librarys（:1148，无地图库名单别名）**、is_map_lib、get_lib_from_string。
  - 编辑器/UI：create_ui_to、get_update_progress_func、get_update_progress_func_clear（:1157-1158）、map_completeness_check（:1161，**打开地图前的项目规格检查**，utils/event.lua:297、map_starter/init.lua:172 消费）、set_tree_data、get_scene_list、get_scene_show_name、get_scenes_show_name、common_prompt。
  - 版本信息：get_current_editor_info、get_map_last_editor_info、set_map_last_editor_info（:1171-1173）。
  - 网络/杂项：concat_pinyin（:1175）、update_http_ip、get_http_ip、is_special_ip、get_doc_url、get_developer_center_url、ASCII、is_lower_letter/is_upper_letter/is_english_letter/is_number_letter（:1182-1186）。
- 依赖：`@common.base.co`（:1）、`ui.components.message_window`（:3）、`log = log`（:2，无意义自保行）。
- 补丁相关：**`EDITOR.test` 为 true 时末尾自动 print_table 自检（:1190-1208）**——加载本文件必打一张测试表到控制台；concat_pinyin 依赖 third-party/lua-pinyin。

## utils/event.lua（核心）
- 用途：**EDITOR 事件系统 + 地图加载/保存/上传管线实现**（全库中枢之一）。
- 导出：无 return；全部挂 EDITOR（:804-815）：`EDITOR.event_register/event_notify/register_componet_event/notify_componet_event/update_map_libs/load_map/load_map_without_check/unload_map/save_map/save_map_progress/upload_map`。
- 依赖：`@common.base.util`（:1）、`@common.base.argv`（:2）、`project_manager`（:3）、`@common.json`（:7）、`window.autotest_app`（:34）、`@base.update.core.api_version_config`（:219-221）、`ui.components.message_window`（:222）、`@common.base.profiler`（:205、:624，函数内延迟 include）、`trigger.trigger_manager`（:295、:531，函数内延迟 require）、`window.trigger_editor_app`（:537，延迟）、`upload_map.upload_map_view`（:659，顶层）。
- 补丁相关（机制详述）：
  - **事件实现**：`editor_events` 局部表（:4）；`event_register = base.event_register(editor_events, name, callback)`（:10-12），`event_notify = base.event_dispatch(editor_events, name, ...)`（:16-18）——同步派发，返回值为最后一个回调结果（save_plugin :439-451 利用此特性收返回值；is_main=false 时改用 `base.promise()` + `promise:co_result()` 协程等待 :441-445）。component_event 表（:20-29）是按组件隔离的第二套（注意 `notify_componet_event` :27 的 `if component_event[self_component_b] then return end` 疑似反逻辑）。
  - **load_map 完整管线**（load_map_impl :57-199）：`MainFrame:SetMap(map_path)`（:70）→ 进度条开始 → EVENT.download_map_ref_libs（:77）→ **EVENT.add_lib_path（:81）→ EVENT.load_map（:82）** → EVENT.trigger_editor_pre_require（:87）→ EVENT.trigger_debugger_init（:88）→ EVENT.trigger_editor_reload（:90）→ EVENT.trigger_validator_editor_reload（:92）→ EVENT.localization_on_loadmap（:105）→ **EVENT.load_map_done（:109）** → EVENT.download_map_resources_from_ref_package（:184）→ 进度条结束 → `require 'ui.common.guide'.show_guide_ui()`（:196）。**补丁挂「打开地图后」逻辑就注册 EVENT.load_map_done**。
  - **外层 load_map**（:292-405）：先 `EDITOR.utils.map_completeness_check`（:297）；`open_subprocess` 且已有图时 `ProcessInfo.process_manager:open_project(map_path)`（:299-305，开子进程）；按项目是否 Lib 发 `EVENT.window_title_bar_register/unregister('设置/项目版本', ...)`（:311-317）；API 版本不一致时弹窗选「编辑器/项目」版本，选项目版本则 `change_api_restart()`（:225-241，拼 cmdline 加 `-editor_api_version=` 后 `common.open_url(launcher, cmdline)` + `common.force_exit()`）；`force_use_editor_api` argv 或参数可强制对齐（:334-338）。
  - **save_map_progress 管线**（:525-621）：备份（:528 backup_obj_and_trigger_editor :503-523，依赖 `_G.objv2_loaded`/`_G.trigger_editor_loaded`，由 :484-499 两个字符串事件 'objv2_loaded'/'trigger_editor_loaded' 维护）→ trigger_manager.save_changes（:550）→ ProjectSettings:save（:552）→ BloodStrip 配置（:553-556）→ EVENT.autosave_end（:562）→ EVENT.update_trigger_v2_obj_time_stamp（:568）→ 依次 save_plugin：obj_editor（:579）→ tile_editor（:585）→ gui_editor（:591）→ trigger_editor（:596）→ mechanism_editor（:602）→ **obj_ref（:608，ref/init.lua 响应）** → localization_on_savemap（:614）；global_default 项目只存 trigger（:571-575）。
  - **upload_map**（:709-800）：未登录直接弹窗（:712-718）；先 EVENT.save_map（:725）→ upload_backup（:727，GetUploadConfig 控制 :668-703）→ 拷贝到 `SCECheckpoint/publish|upload_ref/<地图>_<时间戳>/`（:740-747）→ V2 删 node_modules（:748-750）→ `upload_map_view.upload_target_map/upload_target_map_ref`（:783-784）。
  - **autotest 注册**：update_map_libs/load_map/unload_map 均注册到 autotest_app（:54-55、:407、:802）——自动化测试可重放这些操作。
  - `_G.last_save_as_debug`（:61、:410、:640）、`_G.objv2_loaded`（:202、:488-491）、`_G.trigger_editor_loaded`（:203、:494-499）是跨模块状态全局。

## utils/math.lua
- 用途：颜色工具（color_decode/color_encode/lerp_color）。
- 导出：`EDITOR.math = { color_decode, color_encode, lerp_color }`（:40-44）。
- 依赖：`@common.base.math`（:1，提供 base.math.lerp）。
- 补丁相关：EDITOR.test 自检 lerp_color 四例（:46-55）。

## utils/shortcut.lua
- 用途：**快捷键管理器**：`_G.shortcutMgr`（注册/注销/锁/查询 + 全部快捷键常量）+ `_G.shortcut_events.on_shortcut_pressed` 回调分发。
- 导出：无 return；`_G.shortcutMgr = { register, has_registered, unregister, get_shortcut_pressed, lock, unlock, lock_all, unlock_all, ...常量 }`（:67-166）。
- 依赖：引擎 C++ 全局 `shortcut.register_shortcut/has_register_shortcut/unregister_shortcut/get_shortcut_pressed/lock/unlock/lock_all/unlock_all`（:7-53）。
- 补丁相关：**快捷键常量表**（:78-165）：1001 RELOAD、1002-1008 地编选择/移动/旋转/缩放/相机、1009 UNDO、1010 REDO、1011 UPDATE_ASSIST_GRID_SCALE、1012 SHOW_COLLISION、1013 NEW、1014 OPEN、1015 SAVE、1016 SAVEAS、1017 RELOAD_SHADERS、1018-1039 地编笔刷/雾/指示器/调试运行、1100-1104 数编、1200-1269 触发器 v1/校验器/v2、1300-1303 GUI 编辑器、1400 COMPONENTS_LIB_SAVE、1500-1505 粒子编辑器。窗口分类注释 :57-66（Main/ArtWorkBenchApp/ObjectEditor/TriggerEditor/ProjectManagerApp/LoginWindow/ResourcesManagerApp）。C++ 回调进 Lua 的入口是 `_G.shortcut_events.on_shortcut_pressed(pressed, w)`（:20-30），支持按窗口注册（registered_func[name][window] :9-14）。main.lua:806-901 消费。

## utils/map_download_refs.lua
- 用途：地图依赖下载：响应三个 EVENT，把 libs.json/ref 文件解析成包列表并同步下载。
- 导出：无（事件注册即生效）。
- 依赖：`@common.json`（:1）、`http_requests.goods`（:2）、`@common.base.argv`（:3）、`@common.base.progress`（:6）、`@common.update`（:48）、`@base.update.core.api_version_config`（:49）。
- 补丁相关：**`EDITOR.event_register(EVENT.download_map_ref_libs, ...)`（:175-178，读 `<地图>/libs.json` :106-129）、`EVENT.download_map_ref_resources`（:180-183，读 `ref/editor_full.ref` :131-156）、`EVENT.download_map_resources_from_ref_package`（:185-188，读 `ref_package/editor_ref_package.ref` :158-169）**；`-local_test`/`-no_update` 时全部跳过（has_no_update :171-173）；`MapUpdateRefsBind` class（:10-46）把下载进度绑到 EProgressBar；`aim_version` 指定版本时三方库带版本下载（:53-60）。
