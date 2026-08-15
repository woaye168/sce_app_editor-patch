# script-199 / common/base/ui + common/test + test 逐文件研究记录

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/script-199/` 下的
> - `common/base/ui/`（13 个 .lua，UI 框架核心，详写）
> - `common/test/`（73 个 .lua，测试用例，简写）
> - `test/gui_component/`（13 个 .lua，组件框架测试，简写）
>
> 全部结论来自真实读取，关键结论标注 相对路径:行号。源文件注释为 GBK 编码（乱码已忽略）。

---

# 一、common/base/ui/ —— UI 框架核心

## 加载链

`common/base/ui/init.lua:1-12` 依次 include：`rich_text_custom_tag` → `bind` → `ui` → `event` → `brush` → `control` → `hook`（`auto_scale` 被注释掉不单独 include，由 `ui.lua:924` require 进 `base.ui.auto_scale`）。注意全部用 **include**（引擎热更语义，不走 package.loaded），不是 require。

**`base.ui` 表不是本目录某个文件的 return 值，而是 `ui.lua:906-927` 在全局 `base` 上挂的表**；`base.ui.mt`（ui.lua:740 起）是所有控件实例的元表；event.lua 与 brush.lua 通过 `base.ui.mt` / `base.ui.brush` 继续扩展。

---

## common/base/ui/ui.lua （UI 框架核心中的核心）

- 用途：声明式 UI 框架主模块——控件模板（template）→ 编译创建（create/compile_child）→ 数据绑定（bind/watch）→ 帧驱动落引擎（on_ui_tick → wait_to_create 队列 → C++ `ui.*`）。
- 导出：无 return；挂全局 `base.ui = { ... }`（ui.lua:906-927），字段：
  - `create = create`（:907，签名 `create(template, name, bind, p)` → `ui, bind.api, slot_ui`，定义 :717-738）
  - `map = ui_map`（:908，全部已创建控件 id→实例）
  - `bind = bind_map`（:909，具名 UI 的 bind.api）
  - `gui = gui`（:910，C++ `ui.*` 的动态代理，见下）
  - `mt = mt`（:911，控件元表）
  - `view = view`（:912，快速建顶层视图 :449-461，自动 id `ui-<n>-<name>`，挂到 'main' 下）
  - `watch / set_array / flush / list / template / component / update_event / check_create_new / deep_copy / add_wait_to_create_ctrl / auto_scale / emit_prop_changed / get_ui_at_position`（:913-926）
- 依赖：`include 'base.platform' / 'base.argv' / 'base.profiler' / 'base.co'`（:16-19）、`include 'class'`（:21）、**跨库** `require '@common.base.gui.bibind'`（:23）、`require '@common.base.gui.control_util'`（:25，提供 on_prop_changed/emit_on_created/creating_components_stack/register_as_component_part/get_owner_component/get_final_ext_component/is_component_ctrl/unregister_part/unregister_component_child/is_component_template，:26-35）、`include 'base.ui.image_cache'`（:242）、`require 'base.ui.auto_scale'`（:790、:924）。读写 C++ 全局 `ui.*`（:68 `ui.set_control_prop`、:135 `ui.add_childs_t`、:785 `ui.get_rect` 等）、`common.profile_begin_block`（:147）、`base.event.on_ui_tick`（:270）、`base.proto.__return_check_text`（:889）、`register_reload_event`（:845）。

### component 工厂（ui.lua:584-615）
- `local function component(type_name, base)`：内部类型名加前缀 `'cui_'..type_name`（:586），重名自动加序号（:587-592）。
- 基类取参 `base`（有 `.ctor` 直接用，否则取 `base.__component`），缺省 `include 'base.ui.component.base'`（:594）。
- `class(type_name, base_class)` 建类（:595），并注册进 `ui_creates[type_name] = function(props, bind) ... component.new():__create(props, bind)`（:596-599，第三返回值 true 表示组件）。
- 返回值 `rs` 是自指 metatable 表（:601-614）：`__component` 持有类、`__newindex/__index` 转发到类、`__call(props)` 时给 props 打 `__ui_type` 标记。**补丁可直接调用 `base.ui.component('名字')` 定义新组件，或包装它拦截所有组件注册**。

### template 工厂（ui.lua:561-581）
- `local function template(ui, type_name)`：注册原生控件类型构造器进 `ui_creates[type_name]`，重复注册报 `log.error('repeat register ui:' ...)`（:563-566）。
- 返回的构造器支持两种形态：直接 `fn(props)`（打 `__ui_type`），或 `fn('part_tag')(props)`（额外打 `__part_tags`，:570-580）。
- `base.ui.panel` / `base.ui.label` / `base.ui.button` 等**不在本文件注册**——本文件只提供 `template` 注册函数；具体类型注册在别处（common/test 用法反证其存在，注册点在本镜像外或 C++ 侧）。

### create（ui.lua:717-738）
- `create(template, name, bind, p)`：`init()`（:718，首次调用时建根 panel 'main'，:116-142）→ `bind or base.bind()`（:719）→ `compile_child(template, bind, p)`（:723）→ 具名时登记 `bind_map[name]`/`ui_list[name]`（:732-736）→ 返回 `ui, bind.api, slot_ui`。
- **创建是延迟的**：控件先进 `wait_to_create` 队列（add_wait_to_create_ctrl :442-446），下一帧 `base.event.on_ui_tick`（:270-284）里 `check_create_new()`（:179-217）批量 `gui.add_childs_t` 落引擎并 `subscribe_now()` + `emit_on_created()`。
- 性能护栏：预创建 >100ms 打 `log_file.warn`（:727-731）。

### 控件元表 mt（:740-833，即 base.ui.mt）
- `mt.__index = mt`（:740）；默认字段 `id='未知'/name='匿名'/_visible=true/show=true`（:742-745）。
- 方法：`__tostring`（:747）、`on_tick(callback)`（:755-769，注册帧回调进 tick_map，**返回取消函数**）、`remove()`（:771）、`add_child(child)`（:775，array 控件拒绝 :779-781）、`get_screen_rect()`（:786）、`get_ui_rect()`（:791，除全局缩放）、`rect()`（:800，deprecated）、`xywh(relative_ctrl_or_option)`（:804-829，支持 'root'/'ui_parent'/控件）、`get_image_wh()`（:831）、`set_visible(visible)`（:835）。
- event.lua 继续扩展 `mt:subscribe/unsubscribe/subscribe_now`（event.lua:102-137）。

### 属性 watch / bind 机制
- `watch(ui, template, bind, key, format)`（:243-268）：模板静态值直接赋值；`template.bind[key]` 存在时在 `bind.watch[key]` 上挂回调，值变更时 `gui['set_'..key](ui.id, v)` + `emit_prop_changed`（图片类走 image_cache :247-248）。
- `ui_default`（:475-547）对 29 个通用属性逐一 watch（swallow_event/static/disabled/z_index/clip/show/color/opacity/scale/rotate/focus/image/border 等，:476-504），并深拷贝 event/layout/transition/__EDIT_TIME（:508-511），组件节点走 `register_as_component_part`（:513-519），event 自动 subscribe（:521-525）。
- `gui` 动态代理（:69-94）：任意 `set_xxx` 未在白名单 `is_not_prop_set`（:55-66）时改写为 `_set_control_prop(id, k, ...)` 走通用属性通道；其余原样转发 C++ `ui[key]`。每次调用累加 `callback_count`（:74，base.ui_info() :850 暴露）。
- `flush(mode)`（:427-440）：nil=清 collect 态 UI；字符串=设 flush_state；table=给 UI 打 `_flush` 标记。配合 `register_reload_event('ui_remove_main_children', ...)`（:845-848）实现热更清场。

### 补丁相关（关键 hook 点）
1. **`base.ui.create` 已被 hook.lua 包装一次**（hook.lua:116-120），再包装时注意顺序——hook.lua 是 init.lua 最后 include 的（init.lua:12）。
2. `ui_creates[type_name]` 表（:560）是类型→构造器注册表，可用 `base.ui.template` 注册新控件类型或覆盖既有类型。
3. `base.ui.map` / `base.ui.list` / `base.ui.bind` 是全量控件/具名 UI 登记表，编辑器补丁可遍历找目标控件。
4. `base.ui_info()`（:850-857）暴露 ui_map/tick_map/wait_to_create/callback 计数——调试入口。
5. 事件总入口是全局 `ui_events`（event.lua:298-308 填充），C++ 回调 Lua 的落点。
6. 加载收尾打印 `>>>>>>>>================== init base.ui`（:928）——加载时机日志锚点。

## common/base/ui/bind.lua
- 用途：数据绑定核心——`base.bind()` 工厂，把「字符串表达式路径」编译为 watch 链，值写入时回调 UI。
- 导出：挂全局 `function base.bind(outer)`（:254-263），返回 bind 对象（mt :168-252）：`load(bind)`（:171）、`push(id)/pop()`（:175-197，数组上下文）、`index(n)`（:199）、`compact(id, n)`（:236，数组收缩清理）、`get_state()/switch_state()`（:240-252）、字段 `api/value/array/outer/watch`。
- 依赖：无 require（纯 Lua）；用 `load()` 编译表达式（:18）——**isolation 下 load 被强制 mode='t'，这里本身即是文本模式不受影响**；`base.json.encode` 仅用于报错（:117）。
- 补丁相关：表达式编译 `compile`（:23-30）用 `load('return '..exp)` 在带 `__index` 的 compile_mt 上跑以收集 key 路径（:11-20）；`bind.watch` 是 `__newindex` 代理表（:158-166），**给 `bind.watch.xxx = func` 赋值即触发 watch 注册**——这是 UI 补丁注入「属性变化→回调」的官方通道。

## common/base/ui/event.lua
- 用途：UI 事件系统——定义事件清单、C++ 回调入口 `ui_events`、订阅计数与代理逻辑（长按/拖拽/焦点等）。
- 导出：挂 `base.ui.event = { call, release_event, set_long_click_timeout }`（:314-318）；扩展 `base.ui.mt:subscribe(name)`（:102）、`:unsubscribe`（:115）、`:subscribe_now()`（:128）。
- 依赖：读全局 `base.ui.map/.gui`（:53、:91 等）、`base.wait`（:161）、`base.screen:input_mouse()`（:247）、`base.game:event_notify`（:265）；**写全局 `ui_events[event_name]`**（:298，`init()` :296-310 模块加载即执行 :312）。
- 事件清单 `event_list`（:1-50）：on_click/on_double_click/on_mouse_enter/leave/down/up/on_real_click/on_drag/on_drop/on_focus(_lose)/on_input/on_long_click(_release)/on_update_scroll/on_scroll_rect_changed/on_text_click/on_vj_*(5 个摇杆)/on_virtual_window_*(5 个窗口)/on_spline_curve_*/on_bezier_curve_*/on_color_packer_change/on_color_panel_change/on_web_message。
- 补丁相关：
  - **事件代理表 `proxy`（:175-287）**：on_mouse_enter/leave/down/up/on_click/on_drag/on_drop/on_focus/on_focus_lose/on_input/on_update_scroll 先入 Lua 代理再分发，内含长按计时（:152-173，默认 1000ms）、拖拽位移抑制点击（:211-213）。
  - `proxy_map`（:69-72）把 on_throw/on_dropped 归并到 on_drop；`proxy_pairs`（:73-80）自动订阅配对事件。
  - **可 hook 点：改 `ui_events` 表项即可全局拦截引擎→Lua 的 UI 事件**；`base.ui.event.call`（:52-67）是所有事件最终分发点。

## common/base/ui/hook.lua
- 用途：UI 创建前置钩子（字体替换、r_width/r_height 分辨率自适应），**示范了官方包装 `base.ui.create` 的做法**。
- 导出：无 return；副作用为包装 `base.ui.create`（:116-120：`process_control(data, pre_hooks)` 后调原 create）。
- 依赖：`include 'base.platform' / 'base.argv'`（:2-3）、`common.get_resolution()`（:25）、`base.game:event('画面-分辨率变化', ...)`（:50）、`base.ui_info()`（:66）。
- 补丁相关：`pre_hooks = { hook_font, hook_r_size }`（:105）——**新增全局 UI 钩子只需包装 `base.ui.create` 或往此模式照抄**；hook_font 在微信/QQ/`custom_font` argv 下强制 `font.family='Custom'`（:11-15）；hook_r_size 把 layout.r_width/r_height 换算成固定像素并监听分辨率变化（:20-71）。

## common/base/ui/brush.lua
- 用途：canvas 控件画刷 API 封装。
- 导出：挂 `base.ui.brush = brush`（:72），类方法：`create(canvas_id)`（:6）、`clear/set_line_width/set_line_color/set_fill_color`（:13-27）、`draw_line/draw_circle/fill_circle/draw_polygon/fill_polygon/draw_image/rotate/path_line_to/path_stroke/path_bezier_curve_to`（:29-70）。
- 依赖：全部转发 C++ `ui.*`（如 `ui.draw_line` :30）；`base.json.encode`（:43）。
- 补丁相关：无（纯转发封装；C++ 侧 `ui.*` 才是实现）。

## common/base/ui/auto_scale.lua
- 用途：全局 UI 缩放（按分辨率对参考分辨率 2340x1080 缩放）。
- 导出：`return { set_reference_resolution, get_reference_resolution, set_match_width_or_height, current_scale, disable, enable, set_scale_by }`（:74-82）。
- 依赖：`ui.set_global_scale`（:15）、`base.math.lerp/clamp`（:22、:68）、`base.game:event('画面-分辨率变化', ...)`（:36）、`common.get_resolution()`（:49）。
- 补丁相关：模块加载即 `enable_auto_scale()`（:72）；ui.lua:790 引其 `current_scale` 做坐标换算。

## common/base/ui/image_cache.lua
- 用途：网络图片缓存（供 watch() :247 对 image 属性判 http 链接走缓存）。
- 导出：`return require '@base.base.ui.image_cache'`（:1，**跨库转发桩**，实现在 client_base）。
- 补丁相关：无（桩）。

## common/base/ui/rich_text_custom_tag.lua
- 用途：注册富文本自定义标签（dn/tip/locale/pn），查单位/物品名与描述。
- 导出：无 return；调 `ui.set_rich_text_custom_tag('dn'|'tip'|'locale'|'pn', fn)`（:3、:23、:43、:46）。
- 依赖：`__lua_state_name`（:2，**StateEditor 下整段跳过**）、`base.local_player`、`base.eff.cache`、`base.i18n.get_text`、`base.item/unit`。
- 补丁相关：**`:2` 的 `__lua_state_name ~= 'StateEditor'` 是明确的编辑器/游戏分支点**——编辑器 state 不注册这些标签。

## common/base/ui/component/base.lua
- 用途：`base.ui.component()` 的默认基类 BaseComponent——props 定义/合并/watch、template 构建、生命周期。
- 导出：`return BaseComponent`（:319，`class('base_component')` :91）。
- 依赖：`include 'class'`（:1）、`base.bind`（:7）、`base.ui.create`（:9，缓存为 `ac_ui_create`，:133 调）。
- 关键方法：`__create(out_props, bind)`（:93-161，主流程：__set_instantiation_args→__set_children→define()→after_define()→bind_helper_build→__merge_props_to_root→__build_props→__watch_props→`ac_ui_create` :133→__set_default_props→init()→挂 on_update/on_remove）；`__build_props`（:199-236，props 代理表，setter 签名 `setter(v, old, default, raw_set)` :228）；`__set_default_props`（:258-284，按 priority 升序设初值）；`auto_remove(trigger)`（:304-312）；可重写钩子 `after_define/init/on_remove`（:314-317）、`on_update`（:142 存在才挂 tick）。
- 补丁相关：`bind_helper`（:17-26，`__bind_name` 标记表）+ `bind_helper_build`（:29-72）把 `bind.xxx` 占位符转成 template.bind——组件定义的简写通道；**基类单例，改这里影响所有旧式 component**。

## common/base/ui/component/focus.lua
- 用途：FocusComponent——带焦点语义的组件基类（点自身 on_focus、点别处 on_focus_lose）。
- 导出：`return FocusComponent`（:42，`class('focus_component', BaseComponent)` :4）。
- 依赖：`include 'base.ui.component.base'`（:1）、`base.game:event('鼠标-松开', ...)`（:18）、`base.next`（:19）。
- 补丁相关：占用根 ui 的 on_mouse_down（:15）——继承时根 ui 不可再用 on_mouse_down（:3 注释）。

## common/base/ui/control/init.lua
- 用途：`base.control = {}`（:1）并 include 虚拟摇杆模板（:3）。
- 导出：挂全局 `base.control`。
- 补丁相关：无。

## common/base/ui/control/virtual_joystick_template.lua
- 用途：4 个虚拟摇杆预置模板（移动/移动按下居中/技能/技能按下居中）。
- 导出：`base.control.move_virtual_joystick_template(body, background, slider)`（:20）、`move_virtual_joystick_press_center_template`（:33）、`spell_virtual_joystick_template(body, background, skill_icon, slider)`（:52）、`spell_virtual_joystick_press_center_template`（:69）；均返回 `base.ui.virtual_joystick(body)`。
- 依赖：`base.ui.virtual_joystick` / `base.ui.virtual_joystick_slider`（:28-30 等，注册点不在本文件）；头注释（:1-18）是 vj_* 属性与 on_vj_* 事件的完整文档。
- 补丁相关：无。

---

# 二、common/test/ —— 测试用例（73 个，简写）

> 通用模式：`base.ui.xxx{...}` 建模板 → `base.ui.create(tpl, 'name')` → 改 bind 验证。经 `common/main.lua` 的 `-test` argv 按需 include。均无补丁价值，仅列用途。

- **alpha_blend.lua** — 旧式 `ui.add_child` + json 验证透明度混合（:29）。
- **animation.lua** — transition.show 动画 + bind.show 切换（:9-26）。
- **array.lua** — 嵌套 array 控件（row/col）+ bind 动态改行数（:37-44）。
- **auto_size.lua** — 旧式 json UI：label height=-1 自适应 + 富文本变量（:106-152）。
- **bind.lua** — array 控件 bind event 回调（on_drag 按下标绑定）（:30-43）。
- **bold.lua** — font.bold 绑定（:8-16）。
- **border.lua** — 九宫格 border 图（:4）。
- **canvas.lua** — 旧式 canvas 全绘图 API（draw_line/circle/polygon/rotate/draw_image）（:41-193）。
- **card_in_hand.lua** — p_ui.card_in_hand 手牌控件（:13）。
- **ce_button.lua** — p_ui.button 按钮 + on_click（:3-8）。
- **chat.lua** — label 文本累加模拟聊天（:21-25）。
- **checkbox.lua** — p_ui.checkbox 数组绑定 text/checked/on_checked（:11-22）。
- **children_grow_height_panel.lua** — 滚动面板子控件 grow_height（:13-20）。
- **client_unit.lua** — 鼠标按下 `game.create_unit` 建客户端单位、松开删除（:7-24）。
- **combine_control.lua** — 自定义控件组合（datagrid 内嵌 my_control.define）（:6-24）。
- **custom_control.lua** — p_ui.grid 自定义控件嵌入标准控件（:18-24）。
- **custom_font.lua** — 大字号自定义字体渲染（:7-10）。
- **custom_transition.lua** — transition 自定义贝塞尔曲线 func（:18-25）。
- **custom_value_transition.lua** — bind.transition.custom 自定义过渡（:9-20）。
- **datagrid.lua** — p_ui.datagrid 表格数据展示（:15-20）。
- **drag.lua** — enable_drag + on_drag/on_drop/on_click 绑定（:8-14）。
- **event.lua** — 旧式 `ui_events.on_click` + `ui.unregister_event`（:5-10）。
- **flip.lua** — flip_x/flip_y 绑定（:6-19）。
- **image_cache.lua** — http 网络图片 image 缓存（:2、:7）。
- **inherit_control.lua** — 继承 datagrid 改样式的自定义控件（:5-19）。
- **keyboard.lua** — 旧式 `game_events.on_key_down` 键盘事件（:19-20）。
- **label.lua** — 旧式 json label 颜色/对齐（:18 起）。
- **label_rotate.lua** — font.rotate 旋转文本（:4）。
- **layout.lua** — grow_width/height、width=-1 布局（:3-20）。
- **line_chart.lua** — p_ui.line_chart 折线图（:6-20）。
- **long_click.lua** — on_long_click 与拖拽共存（:11-19）。
- **margin_percent.lua** — margin_percent/padding_percent 百分比边距（:8-17）。
- **mask.lua** — mask_image 遮罩（:8、:18）。
- **mouse_event.lua** — 鼠标事件显示按键名（:17-19）。
- **multi_line_edit.lua** — input font.multi_line 多行输入（:1-12）。
- **npot_texture.lua** — 非 2 幂网络纹理（:7）。
- **panel.lua** — 滚动面板 + array 按钮（:11-17）。
- **particle.lua** — scene 控件内 model+particle（:8-16）。
- **position.lua** — relative 相对定位（:13）。
- **progress.lua** — progress 控件环形进度 + base.loop（:4-14）。
- **r_size.lua** — layout.r_width/r_height 分辨率比例尺寸（:11-19）。
- **random_panel.lua** — 横向滚动 + 100 随机子控件压力（:14-20）。
- **ratio.lua** — ratio 比例布局（本镜像仅 9 行，:1-9）。
- **read_table.lua** — `game.GetGameTable` 读数编表 + table_writer 打印（:3-8）。
- **resolution.lua** — 旧式 json 显示当前分辨率（:14-19）。
- **rotate.lua** — rotate 属性持续旋转（:16-19）。
- **scale.lua** — array 玩家面板布局（:15-20）。
- **scene.lua** — scene 控件 model 动画绑定（:8-19）。
- **scenes.lua** — 旧式 json 多 scene 并排（:19-20）。
- **select.lua** — p_ui.select 下拉选择（:11-19）。
- **size_animation.lua** — transition.size/position 尺寸动画（:13-19）。
- **sleep.lua** — 协程 sleep 工具（`execute` + `game_events.on_update` 驱动）（:10-20）。
- **spine.lua** — spine 动画 + select 切换（:17-19）。
- **sprites.lua** — sprites 序列帧控件参数演示（:2-20）。
- **table_writter.lua** — `common.test()` 引擎接口返回表打印（:1-5）。
- **test_ac.lua** — scene 控件嵌套面板（:14-19）。
- **timer.lua** — base.wait 百万级定时器压力基准（:11-19）。
- **timer2.lua** — `timer.loop` 全局定时器 + 移除（:4-15）。
- **timer3.lua** — co.async 内 `timer.wait`（:3-8）。
- **tracer.lua** — base.tracer/profiler 性能追踪（:1-2、:12-19）。
- **transition-bezier.lua** — 旧式 json transition position 贝塞尔（:15-18）。
- **transition-bezier2.lua** — 血条 label transition（:15-19）。
- **transition.lua** — 旧式 json transition + `require 'test.sleep'`（:2、:15-18）。
- **transition_curve.lua** — transition func='curve' 锚点曲线移动（:2-18）。
- **transition_progress.lua** — progress 值过渡动画（:6-11）。
- **translate.lua** — layout.translate 比例偏移（:16）。
- **translate_array.lua** — array + translate 组合（:6、:18-20）。
- **tree.lua** — p_ui.tree 树控件递归数据（:7-17）。
- **update.lua** — `base.update` 热更新流程（:7-18）。
- **vertical_align.lua** — font.vertical_align 垂直对齐（:9）。
- **virtual_joystick.lua** — `base.control.move_virtual_joystick_template` 摇杆（:1-12）。
- **wx_game_pay.lua** — 微信支付 `wx.pay`（:8-16）。

---

# 三、test/gui_component/ —— 组件框架测试（13 个，简写）

> 通用模式：`require '@common.base.gui.component'`（**跨库**，实现在 client_base/gui 库）+ `component {...}`/`component '名字' {...}` 定义组件 + `check()` 断言。这是**新版组件框架**（区别于 base/ui/component/base.lua 旧式 component），main.lua 是入口。

- **main.lua** — 测试入口：定义全局 `check(b, msg, ...)` 断言（:3-16），`auto_test` 数组逐个 require 用例（:30-40+：list/general_usage/extend/part/simple_panel/template/event/prop/move_ctrl/compatibility 等）。
- **anim.lua** — `@common.base.anim` 关键帧动画 set/stop（:3-13）。
- **compatibility.lua** — 组件 bind/bibind/array 与旧 bind 兼容性（:12-25）。
- **destroy.lua** — 组件 on_destroy 生命周期、move_to_new_parent（:14-25）。
- **event.lua** — 组件 event 声明 + key_frame_state/anim_trans（:16-25）。
- **extend.lua** — 组件 prop 继承/bind 表达式 `bind 'on_click'`（:8-24）。
- **general_usage.lua** — 组件一般用法：命名/匿名、get_ctrl_type_name、`:new('inst_name', {props})`（:8-25）。
- **list.lua** — `@gameui.component.virtual_list/virtual_table` 虚拟列表（:2-14）。
- **move_ctrl.lua** — `move_to_new_parent` 控件跨父移动（:9-25）。
- **part.lua** — 组件嵌套 part（A 嵌 B，name 定位）（:9-25）。
- **prop.lua** — prop 系统：bind.text 绑定、struct_prop、bibind 已废除注释（:4、:15-25）。
- **simple_panel.lua** — 完整面板组件示例（image 绑定、子按钮）（:10-25）。
- **template.lua** — 组件 method/slot 模板继承（:11-25）。

---

# 四、补丁视角总结

1. **UI 框架分两代**：`base.ui.component()`（ui.lua:584 + component/base.lua，旧式 class 组件）与 `@common.base.gui.component`（跨库新式组件，test/gui_component 全用它；ui.lua 通过 `control_util` :25-35 与 bibind :23 与新框架互通）。编辑器 UI 补丁需先确认目标 UI 用哪代。
2. **三大全局拦截点**：`base.ui.create`（ui.lua:717，已被 hook.lua 包装，再包装注意链序）、`base.ui.template`/`ui_creates` 类型注册表（ui.lua:561）、全局 `ui_events` 表（event.lua:298，C++→Lua 事件唯一入口）。
3. **创建是帧延迟的**（wait_to_create → on_ui_tick → check_create_new，ui.lua:442/270/179）：补丁里 create 后立刻按 id 查 `base.ui.map` 会拿不到，需等下一帧或用 `base.ui.list` 具名登记。
