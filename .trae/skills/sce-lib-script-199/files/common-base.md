# script-199 / common/base 逐文件研究记录

> 研究对象：`D:/sce_online/Res/maps/bgd_glzy/.editor_src_mirror/script-199/common/base/` 根级 111 个 .lua（**不含 ui/ 子目录**，ui/ 另有 9 个文件：init/ui/bind/event/hook/brush/auto_scale/image_cache/rich_text_custom_tag，待另文研究）。
> 全部结论来自真实读取，关键结论标注行号。源文件注释为 GBK 编码（个别乱码已忽略）。

## 总览

### 加载链

`common/base/init.lua` 是 base 目录的总装配入口（由 client_base 侧链路拉起，`init.lua:2` 日志 `==== client base script load start ====`）：

1. `init.lua:1` `require 'base.log'`（日志增强，最先加载）。
2. `init.lua:36-60` 建立核心全局：`_G.base = {}`、`base.error`（:38，test 模式下 :44-53 改写为 log.error + debug_bp）、`base.game`/`base.Game`、`base.game.lni = require 'lni_loader'`（:57，C++ 实现）、**`_G.game_events`/`_G.ui_events` 经 `safe_callback`（:10-34）包装**——写入的每个回调都被 xpcall(base.error) 包裹，`base.event = game_events`（:60）。
3. `init.lua:68` `base.tsc = require 'base.lualib_bundle'`（TypeScriptToLua 运行时）。
4. `init.lua:69-97` 第一批：utility(math/vector/obj_check/event/trigger/timer/json/point/line/collision_flags/scene_point/position/game/terrain/screen/settings/shortcut/algorithm/deque/event_deque/try/exception/promise/localization/area/单位组/thirdordermatrix/friend。注意混用 `require` 与 `include`（include 的不走 package.loaded 缓存）。
5. `init.lua:100-131` **仅非 app 平台**（`platform.is_app()` 为假）加载玩法层：table/eff/eff_param/cmd_result/unit/snapshot/response/actor/anim_handlers/player/skill/buff/team/group/force/hashtable/array/server/item/select_hero/target_filter/quest/slot/riseletter/behavior/circle/rect/margin。
6. `init.lua:134-142` `require "base.ui"`（解析到 `base/ui/init.lua` 子目录入口）、template/ad/voice/select_indicator/cheat/startup/open_url_wrap。
7. `init.lua:147-155` p_ui/error_info/wx，非 app 平台另加载 pay/shell。
8. `init.lua:161-165` **仅 StateGame**：trigger_editor_v2、`require_folder 'base.base_lua_plus'`（C++ 注册的 require_folder）、game_result。

### 转发桩模式（31 个文件）

base/ 根下 31 个小文件是**一行 `return require '@base.base.xxx'` 的跨库转发桩**（@base 前缀指向 client_base 库，不在本镜像），实现已迁往 client_base。涉及：**argv、path、util、platform、co、json、class、try、web、sdk、ip、toast、deque、request、confirm、account、progress、check_log、exception、file_mutex、disconnect、event_deque、localization、table_writer、upload_log、wx、replay、console、json_load、json_save、anim**（anim 桩稍有不同，见条目）。任务重点关注的 argv/path/util/platform/co **全部不在本镜像**，需另研究 client_base 库。

### 关键全局（补丁视角）

- `base` 表是一切 Lua 侧 API 的总命名空间（init.lua:36 创建）；`base.proto`（server.lua:246）是服务器→客户端消息分发表；`base.event`（= C++ 回调表 game_events，init.lua:60）是引擎→Lua 事件入口表；`base.trig`（trigger.lua:20）是触发器体系。
- `log`/`log_file`/`game`/`common`/`sdk`/`sce`/`cmsg_pack`/`include`/`require_folder`/`ImportSCEContext` 均为 C++ 预置全局。
- `base.error`（init.lua:44-53）是所有引擎回调的错误兜底，可 hook 做错误采集。
- `base.debug_bp = debug_bp`（trigger.lua:1440）——`debug_bp` 为 C++ 注入的断点函数（仅在提供时非 nil），log.lua:106-110、init.lua:50 均有调用。

---

## common/base/init.lua
- 用途：base 目录总装配入口，建立 `base`/`game_events` 等核心全局并按平台/state 分批加载全部 base 模块。
- 导出：无（执行型脚本）。
- 依赖：见「加载链」；`require 'lni_loader'`（:57，C++ 模块）、`require_folder 'base.base_lua_plus'`（:163，C++ 全局）。
- 补丁相关：**入口插槽候选**——:1 之前插入可在任何 base 模块加载前执行；:67（`base.tsc` 就绪）后插入可获得完整 TS 类运行时。`safe_callback`（:10-34）证明所有 `game_events`/`ui_events` 回调都被 xpcall 保护，补丁回调抛错不会炸引擎。

## common/base/log.lua
- 用途：log 增强——给 C++ 预置的 `log` 补 format 系列函数，StateEditor 下把 `log.error` 改成弹窗。
- 导出：无 return；全部副作用在全局。`_G.fmt = f:format(...)`（:26-30）；`log.debugf/infof/warnf/errorf(fmt, ...)`（:32-60，仅当 `log.debugf` 不存在时定义）；`log.alertf`（:75）；`log.fail(info)`（:82，StateEditor 走 EMessageBox，否则 log.error）；`log.failf`（:91）；`_G.printf`（:99）；`log.traceback_debug_bp`（:106）；`log_file` 兼容兜底（:114-116，注释明确「log_file挪到C++了」）。
- 依赖：`include '@common.base.argv'`（:12）、`require '@base.base.message_box'`（:17，跨库）、`ImportSCEContext():GetEMessageBox()`（:8-10，仅 StateEditor）。
- 补丁相关：**argv 含 `debug` 或 `lua_debug` 时 StateEditor 的错误弹窗被禁用**（:14-16）。:62-73 StateEditor 下 `log.error` 被改写为「log.info + traceback + message_box 弹窗」——这是编辑器里 Lua 报错弹窗的来源，hook 点明确。本文件是 base 加载链第一个模块（init.lua:1），早于 isolation 之后的一切。

## common/base/timer.lua
- 用途：帧计时器系统（游戏逻辑帧驱动，单位=毫秒帧计数），`base.wait/loop/timer/next` 全家桶。
- 导出：`return { Timer = Timer }`（:516-518）；同时挂全局类 `Timer`（:174）与 base 函数群。
- 关键签名：
  - `base.wait(timeout_ms, on_timer) -> Timer`（:349/:293，inner argv 版本多记 stack_info）
  - `base.loop(timeout_ms, on_timer) -> Timer`（:362/:310，周期<1 帧报错）
  - `base.loop_lazy(timeout, on_timer)`（:376，loop + loop_lazy 标记，帧对齐）
  - `base.next(cb)`（:384，固定 2 帧后执行，经 nexts 表）
  - `base.timer(timeout, count, on_timer)`（:392，count=0 转 loop）
  - `base.uwait/uloop/utimer(u, ...)`（:432-451，挂到对象 `_timers`，10s 一次清扫）
  - `base.clock() -> cur_frame`（:157）；`base.set_timer_warning(w)`（:454）；`base.timer_info()`（:509）
  - Timer 方法：`remove/pause/resume/restart/get_current/get_current_time/set_current_time/get_remaining_time/get_remaining_time_new/set_remaining_time`（:184-290）
- 依赖：`require 'base.profiler'`（:1）、`require 'base.argv'`（:2）。
- 补丁相关：**驱动源是引擎回调 `base.event.on_update(delta)`（:479）→ `base.event.on_tick`（:161）**，逐帧推进 `cur_frame` 并派发 `游戏-更新` 事件（:468）、`on_ui_tick`（:471）、`base.next` 队列（:474）。单帧计时器回调耗时超 `warning`（默认 100ms）会 print 警告（:457-466）。argv 有 `inner` 时 F10 可 dump 全部计时器调用点（:327-347）。可 hook 点：`base.event.on_update` 是每帧第一现场。

## common/base/game.lua
- 用途：客户端游戏实例主体——输入（键鼠/摇杆）事件转发、选择器、镜头、广播、加载/进退前台等引擎事件落地。
- 导出：无 return（include 型脚本，`init.lua:82` 以 include 加载）；作用在 `base.game` 与 `base.event` 上。
- 关键签名（`base.game:`）：`hotkey()` :128、`key_state(key)` :140、`selected_unit()` :148、`chat(type,msg)` :152、`set_game_scene(...)` :167、`get_current_scene()` :172、`lock_camera/unlock_camera()` :177/:181、`set_camera_attribute(k,v,time)` :185、`input_mouse()` :189、`loading_left()` :195、`select_unit(unit)` :204、`circle_selector(pos,radius,tag,ignore_center)` :210、`line_selector(pos,len,width,face,tag)` :253、`sector_selector(...)` :295、`get_winner()/get_winner_team()` :337/:341、`send_broadcast(...)` :347、`camera_focus(unit)` :357；`base.game.get_default_unit(node_mark)` :373（协程等待服务端回包，配 server.lua:267 的 `__return_default_unit`）。
- 依赖：`require 'base.argv'`（:24）。
- 补丁相关：`base.event.on_*` 引擎事件桥（:418 起 60+ 个）：键盘 `on_key_down/on_key_up`（:564/:581，经 key_map 映射 :26-70）、鼠标 `on_click/on_mouse_down/up/move/on_wheel_move`（:511-627）、摇杆（:637-684）、`on_enter_game`（:695）、`on_game_result`（:718）、`on_load_scene`（:724）、`on_game_exit`（:810）、`on_game_kick`（:820）等。要拦截输入/生命周期事件，hook 这些函数即可。

## common/base/event.lua
- 用途：事件系统——`base.game:event/event_notify/event_dispatch/broadcast` 实现、事件序列化（跨端转发）、预设 TS 事件类。
- 导出：无 return；作用在 base 上。
- 关键签名：
  - `base.assign_event(name, f)`（:85）；`base.event_subscribe_list`（:113）
  - `base.event_register(obj, name, f)`（:319）——对象级订阅
  - `base.event_notify(obj, name, ...)`（:282）——核心派发；内部 `__client_event_to_server`（:263）处理转发服务端
  - `base.event_dispatch(obj, name, ...)`（:128）；`base.forward_event_register(name)`（:124）
  - `base.event_serialize(t, depth, event_name)`（:167）/`base.event_deserialize(t)`（:214）——跨端事件参数编解码
  - `base.game:event(name, f)`（:333）、`event_notify`（:329）、`event_dispatch`（:325）、`broadcast(name, f)`（:337）
  - `base.custom_event_notify(event_name, event_param)`（:345）、`base.send_custom_event(event)`（:350）
- 依赖：无 require（依赖 init.lua 先建好 base.game/base.trig）。
- 补丁相关：:7-18 `dispatch_events` 与 :20-80 `notify_events` 两张表列出全部引擎级事件名（单位/技能/玩家/游戏/对话等），是事件 hook 的总目录。:362 起定义大量 `base.<中文名>` TS 事件参数类（单位进入视野/消息技能/游戏开始/游戏结束/玩家断线/按键松开……），触发器体系消费。

## common/base/trigger.lua
- 用途：触发器（Trigger）体系——事件订阅、场景化事件、事件参数构造表。
- 导出：`return { Trigger = Trigger }`（:1601-1603）；全局类 `Trigger`（:13）、`base.trig = Trigger.prototype`（:20）。
- 关键签名：`base.trigger(event, callback)`（:116）、`base.trig:new(action, combine_args, scene, sync)`（:131）、`base.trigger_new_from_function(func)`（:181）、`base.each_trigger()`（:110）、`base.trigger_size()`（:100）；Trigger 方法 `disable/enable/is_enable/__call/remove`（:44-80）。`base.trig.event`（:446）+ `evt.event_list`（:1444，事件名→参数构造器映射总表）+ `base.trig.event.evt_args`。
- 依赖：`require 'base.scene'`（:10）。
- 补丁相关：trigger_map 为弱表（:9）；`base.debug_bp = debug_bp`（:1440）。监听 `游戏-开始`/`游戏-结束`/`场景-加载完成`（:344-356）做触发器清理。

## common/base/server.lua
- 用途：客户端↔服务器消息层——`base.game:server` 发包、`base.proto` 收包分发表、s2c_rpc。
- 导出：`return proto`（:338）；同时 `base.proto = proto`（:246）。
- 关键签名：
  - `base.game:server(type) -> fun(args)`（:184-191）——cmsg_pack.pack 后 `game.send_ui_message`（C++）
  - `base.event.on_ui_message(str)`（:193）/`on_ui_message_new(str, type_id, type_name)`（:214）——收包入口，xpcall(proto[type], base.error)，单包处理超 3ms 打日志
  - `proto.s2c_rpc(data)`（:168）+ s2c_fmap 注册表（:71-166，actor/unit 远端调用白名单）
  - 预置 proto：`reload`（:21，调 C++ `reload()`）、`bind`（:25，服务器写 UI 绑定）、`subscribe`（:49）、`clock`（:67）、`__server_event_to_client`（:250）、`__return_default_unit`（:267）、`__unit_try_pick_item_result`（:291）、`__item_try_drop_result`（:302）、`__add_attribute_and_sync_client`（:314）、`_set_game_speed`（:322）、`__set_attribute_custom_format`（:333）
- 依赖：`require 'lni'`（:1）、`include 'base.profiler'`（:3）。
- 补丁相关：**`base.proto` 是所有服务器→客户端消息的 hook 总表**，补丁可直接 `base.proto.xxx = function` 注册新消息；`base.event.on_ui_message` 是收包第一现场。

## common/base/rpc.lua
- 用途：简易 RPC（经 `__simple_rpc__` 消息走 server.lua 通道），支持函数参数自动转回调 id。
- 导出：`return rpc`（:71）——metatable 表，`rpc.xxx(...)` 即发调用、`rpc.xxx = func` 即注册实现（:14-23）。
- 依赖：`require 'base.server'`（:2）。
- 补丁相关：`rpc.callback(id, ...)`（:62）；函数参数序列化为 `{__rpc_cb__ = id}`（:30-33）。

## common/base/promise.lua
- 用途：promise/multi_promise 异步原语（基于 event_deque + co）。
- 导出：`return { promise, multi_promise, as_promise }`（:231-235）；同时挂 `base.promise`/`coroutine.promise`、`base.multi_promise`/`coroutine.multi_promise`、`base.as_promise`/`coroutine.as_promise`（:222-229）。
- 关键签名（EmmyLua 注解 :30-41 完整）：`promise()` 构造（:134）；`get(timeout, callback)`、`co_get(timeout)`、`co_result(timeout)`（err 时抛异常 :72-79）、`set/try_set/set_result/try_set_result/set_error/try_set_error/ready`。`multi_promise(list, join_type, timeout)`，join_type ∈ `all_finish|any_finish|any_failed`（:147、:193）。`as_promise(f, ...)`（:207）。
- 依赖：`require 'base.event_deque'`（:22）、`require 'base.co'`（:23-24，转发桩→client_base）、`require 'base.exception'`（:25，桩）。
- 补丁相关：无直接 hook 点；是补丁写异步代码可复用的基建。

## common/base/update.lua
- 用途：地图热更新客户端协议（广播 `update_map`/`remove_map`，回执 `update_map_finish`）。
- 导出：`{ start(args), start_async(args), remove(maps) }`（:57-61）；文件头 :1-24 有用法注释。
- 依赖：`include 'base.co'`（:26，桩）。
- 补丁相关：`base.game:send_broadcast('update_map', args)`（:40）是更新指令出口。

## common/base/utility.lua
- 用途：工具函数壳——主体转发 client_base，本地只加 hash 与枚举查询。
- 导出：`return require '@base.base.utility'` 的结果（:1、:32）；另置全局 `Target = utility.Target`（:2）。
- 本地新增：`base.hash(str)`（:4，djb33）；`base.get_appendable_enum(key)`（:14）/`base.get_appendable_keys(key)`（:23）——读 `@<__MAIN_MAP__>.obj.constant`。
- 依赖：`@base.base.utility`（跨库）。
- 补丁相关：:33 起有大段块注释旧实现（io.load 已禁、base.split/string_format/utf8_sub/to_type/get_unit_name/image_path/gc/calc_http_server_address 等），**这些函数的运行时版本在 client_base**，镜像内不可考，仅作历史参考。

## common/base/math.lua
- 用途：角度制数学库 + 浮点比较 + 向量运算，挂 `base.math`。
- 导出：无 return；`base.math = {}`（:1）下：sin/cos/tan/asin/acos/atan（角度制 :17-49）、ceil/floor、float_eq/ueq/lt/le/gt/ge（1e-5 精度 :64-86）、random_float/random_int（:89/:104）、is_int、float_modf、included_angle（:119）、lerp（:132）、clamp（:141）、max/min、vector_add/sub/mul/dot_product/cross_product、sqrt/log/pow/square/exp/abs。
- 补丁相关：:3-9 **用 C++ 的 `cpp_rand.random/randomseed` 覆盖标准 `math.random/randomseed`** 并清掉 cpp_rand——随机数全局被替换，补丁若依赖 math.random 注意此点。

## common/base/algorithm.lua
- 用途：table/math 标准库扩展（EmmyLua 生成风格）。
- 导出：无 return。`table.unique(t,i,j)`（:13）、`table.merge(dst,src,array_append,depth)`（:43-61）、`table.erase(t, func_or_elem, start, stop)`（:63）、`math.min_max(a,b)`（:81）、`math.max_min(a,b)`（:89）。
- 补丁相关：无。

## common/base/array.lua
- 用途：带默认值/长度维护的数组容器 `base.array(default, t?)`。
- 导出：无 return；`base.array`（:98）。实例方法：`set_len/insert/remove/random/convert/ipairs`（:104-109、:37-96），索引 ≤0 报错（:4、:16），`__len`/`__pairs` 支持。
- 补丁相关：无。

## common/base/table.lua
- 用途：数编（数据编辑）表访问层——`base.table` 懒加载元表 + 各类查询函数。
- 导出：无 return；`base.table`（:28，__index 时 `game.get_game_table(name_map[name] or name)` 拉 C++ 侧数编，skill/spell/buff/unit 额外 merge `@@.obj.*` 生成模块 :44-80）；查询函数 `base.skill_table(name,level,key)` :176、`base.unit_table(name,key)` :188、`base.buff_table` :208、`base.attack_table` :220、`base.item_table` :240、`base.spell_table` :253。
- 依赖：`require 'lni'`（:1）。
- 补丁相关：name_map（:3-17）是数编类型→C++ 表名映射（SpellData/UnitData/ActorData/ClientBuff/Constant/ItemData……）。`base.table.config.player_setting` 被 team/force 等模块消费。

## common/base/eff.lua
- 用途：效果（Effect）缓存与枚举——数编节点缓存 `base.eff.cache`、e_cmd 结果码。
- 导出：无 return；`base.eff = {}`（:41）。枚举：`e_cmd`（:45-68，Unknown=-1/OK=0/...）、`e_cmd_str`（:70）、`e_site`（:91）、`e_sub_name`（:105）、`e_target_type`（:114）、`e_stage`（:120）。
- 关键函数：`eff.init_cache()` :134、`eff.merge_cache(in_cache)` :155、`eff.has_cache_init()` :166、`eff.cache_init_finished()` :170、`eff.caches(node_type)` :176、`eff.all_caches(node_type)` :180、`eff.cache(link)` :205、`eff:cache_ts(link)` :215、`eff.get_node_type(node_type)` :219、`eff.cache_as(link, node_type)` :225、`eff.original_data()` :235、`eff.get_namespace(link)` :242、`eff.find_sibling(link, name)` :250、`eff.validate(ref_param, do_cache)` :260、`eff.execute_validators(...)` :286。
- 补丁相关：`base.eff.cache(link)` 是全库读数编缓存的统一入口（`$$.xxx.root` 形式 link）；`Src-PostCacheInit` 事件（camera/behavior 等监听）标志缓存就绪。

## common/base/eff_param.lua
- 用途：效果参数对象（EffectParam/EffectParamShared）——技能/效果执行时的参数载体与阶段（start/channel/shot/finish，:4-9）。
- 导出：`return { EffectParam, EffectParamShared }`（:1308-1311）；`base.eff_param`（:17）、`base.eff_param_shared`（:20）两个 prototype。
- 补丁相关：纯玩法数据结构；大量 EmmyLua 注解（:23 起 EPLoopData/EPSearchData/EPOrderData/EBuffData 等）可作为触编类生成素材。

## common/base/point.lua
- 用途：二维点（带高度/场景）类 Point 及点运算元方法。
- 导出：`return { Point = Point }`（:246-248）；`base.point(x,y,z,scene?)`（:228、:31）、`base.table_to_point`（:229）、`base.get_scene_point(scene, area_name, present)`（:231，取地编点）。
- 关键：元方法 `__add`（:104）、`__sub`（:116，极坐标位移 {angle,distance}）、`__mul`=distance（:127）、`__div`=angle（:133）、`__unm`、`__call`=get_xy（:74）；方法 `get_xy/get_x/get_y/get_height/copy/copy_to_scene_point/get_position(世界转屏幕 :99)/polar_to/polar_to_ex/angle/distance/to_coordinate/is_block`(:202)。
- 依赖：`base.math.cos/sin/atan`。
- 补丁相关：无。

## common/base/scene_point.lua
- 用途：带场景的点 ScenePoint（继承 Target 而非 Point，:11-12 注释明确），支持错误点标记。
- 导出：`return { ScenePoint = ScenePoint }`（:400-402）；`base.scene_point(x,y,z,scene,error_mark?)`（:46、:29）、`base.scene_point_by_hash(x,y,z,scene_hash,error_mark?)`（:47、:41）。
- 依赖：`require 'base.point'`（:4，复用其 prototype 作 metatable 基 :18）、`base.get_scene_hash_by_name`/`get_scene_name_by_hash`。
- 补丁相关：`error_mark` 错误点机制（:50-80，错误点取坐标返回 nil+true）。

## common/base/position.lua
- 用途：屏幕坐标 ScreenPos 类。
- 导出：`return { ScreenPos = ScreenPos }`（:60-62）；`base.position(x?,y?)`（:51）、`base.screen_pos(x,y)`（:56）、`base.mouse_screen_pos()`（:46）。
- 关键：`get_xy/get_x/get_y`、`get_ui_x/get_ui_y`（:33-39，除以全局 UI scale）、`get_point()`（:41，屏幕转世界 `game.screen_to_world`）。
- 依赖：`require '@common.base.ui.auto_scale'.current_scale`（:3）。
- 补丁相关：无。

## common/base/line.lua
- 用途：折线 Line 类 + 地编线获取。
- 导出：`return { Line = Line }`（:38-40）；`base.line(points)`（:21）、`base.get_scene_line(scene, area_name, present)`（:26）。
- 补丁相关：无。

## common/base/circle.lua
- 用途：圆形区域 RegionCircle 类。
- 导出：`return { RegionCircle = RegionCircle }`（:72-74）；`base.circle(point, range, scene_name?)`（:66）。
- 关键：`get_point/get_scene_point/get_range/random_point/scene_random_point`；`init_region(filter)`（:33，建 region 并挂 `区域-进入/区域-离开` 事件 :40-49）、`remove_region`（:55）。
- 依赖：`base.region.circle`（C++/他模块）。
- 补丁相关：无。

## common/base/rect.lua
- 用途：矩形区域 RegionRect 类。
- 导出：`return { RegionRect = RegionRect }`（:124-126）；`base.rect(p1,p2,scene?)` 或 `base.rect(point,width,height,scene?)` 两种重载（:92-122）。
- 关键：`init_region`（:52，转 polygon region 挂区域事件）、`random_point`、`get_start_point` 等。
- 补丁相关：无。

## common/base/area.lua
- 用途：区域查询工具函数集（base.get_scene_area / 区域内单位筛选 / 点区域判定）。
- 导出：无 return。`base.get_scene_area(scene, area_type, area_name, present)`（:3）；`base.get_area_player_type_unit_group(area, player, unit_id_name, filter)`（:16）；`base.get_area_unit`（:35）/`get_circle_area_unit`（:45）/`get_rect_area_unit`（:54）；`base.is_unit_in_area`（:79）/`is_point_in_circle`（:93）/`is_point_in_rect`（:112）/`is_point_in_area`（:132）。
- 补丁相关：含 `---@ui`/`---@description` 等触编导出注解（:68-72、:94-98）——是触编函数 JSON 的直接来源。注意 :11-14 的 `base.circle` 是死代码（引用了不存在的局部 mt/default_scene）。

## common/base/region.lua
- 用途：Region 基类占位（空壳，只有 type='region'）。
- 导出：`return { Region = Region }`（:11-13）；全局 `Region`（:3，base.tsc.__TS__Class()）。
- 补丁相关：无。

## common/base/collision_flags.lua
- 用途：碰撞标志位掩码类。
- 导出：无 return；`base.collision_flags(mask)`（:31）。标志表（:8-20）：Unwalkable=0x2/Unflyable=0x4/Unbuildable=0x8/UnPeonHarvest=0x10/Blighted=0x20/Unfloatable=0x40/Unamphibious=0x80/UnItemplacable=0x100/Cliff=0x200/Higher=0x400/Lower=0x800。方法 `contains(flag)`（:36）、`each_collision(callback)`（:42）。
- 补丁相关：无。

## common/base/vector.lua
- 用途：三维向量 Vector 类。
- 导出：`return { Vector = Vector }`（:48-50）；`base.vector = create_vector`（:46，{X,Y,Z}）。方法：`vector_addition/subtraction/multiplication`（:13-25，注意 :14 有 bug——Y 分量错用 VectorB.X）、`get_vector_length`（:33）、`get_unit_vector`（:40）。
- 补丁相关：无。

## common/base/thirdordermatrix.lua
- 用途：三阶矩阵类（加减/矩阵乘/向量乘/行列式）。
- 导出：`return { ThirdOrderMatrix = ThirdOrderMatrix }`（:85-87）；`base.third_order_matrix = create_tom`（:83，参数为 3x3 二维数组）。
- 补丁相关：无。

## common/base/hashtable.lua
- 用途：弱键哈希表（编辑器哈希表物件，k1/k2 两级，带类型检查）。
- 导出：无 return；`base.hashtable()`（:64）、预建 `base.Hashtable`（:68）。方法 `save(k1,k2,tp,value)`（:11）/`load(k1,k2,tp,def)`（:27，类型不符 error）/`flush`（:48）/`flush_parent`（:52）/`flush_child`（:56）。
- 补丁相关：无。

## common/base/group.lua
- 用途：弱引用对象集合 `base.group(list?)`（insert/remove/has/len/random/ipairs/clear，:8-57）。
- 导出：无 return；`base.group`（:59）。
- 补丁相关：被 force.lua 复用。

## common/base/force.lua
- 用途：势力（玩家组）——`base.force(list)` 可调用表 + 预建 all/computer/user/team 分组。
- 导出：无 return；`base.force = {}` + `__call`（:35-40）。加载即 `init()`（:45-85）：按 `base.table.config.player_setting` 建 `base.force.all/computer/user/team[team]`。
- 补丁相关：加载时机依赖数编 player_setting 就绪；StateEditor 无此表时各分组为空（:51 判空）。

## common/base/team.lua
- 用途：队伍 Team 类（get_id/each_player）+ `base.team(id)` 懒初始化。
- 导出：`return { Team = Team }`（:50-52）；`base.team(id)`（:45）。
- 补丁相关：无。

## common/base/player.lua
- 用途：玩家 Player 类（496 行）——属性/英雄/事件、玩家注册表。
- 导出：`return { Player = Player }`（:496-498）。`base.local_player()`（:327）、`base.player(id)`（:343）、`base.each_player(type?)`（:350）。
- 关键：字段 `_id/_user_id/_ptype/_team/_name/_hero/_online/_vip_level/_loading_progress...`（:8-22）；`init_one_player`（:34）调 C++ `game.get_player_info(id)`（:44）。引擎回调 `base.event.on_player_table_attributes_changed`（:396）/`on_player_attributes_changed`（:407）/`on_loading_progress_notify`（:456）。
- 补丁相关：无直接 hook 点；`base.local_player` 是补丁判断本机玩家的入口。

## common/base/unit.lua
- 用途：单位 Unit 类（2170 行，最大玩法类之一）——继承 Target（:6），属性/技能/物品/状态机封装。
- 导出：`return { Unit = Unit }`（:2170-2172）；`base.unit(id)` 查 unit_map（:24）。
- 依赖：`require 'base.state_machine'`（:1）。
- 补丁相关：unit_map/visible_units/node_mark_map（:24-27）是单位注册表；`mt._statemachines`（:17）配 state_machine。

## common/base/skill.lua
- 用途：技能 Skill 类（1063 行）——槽位/冷却/属性 key 映射。
- 导出：`return { Skill = Skill }`（:1063-1065）；`base.skill_api = Skill.prototype`（:7）。
- 关键：key_map（:11-33，数编序号→字段名 cost/cool/range/_level...）、SLOT_TYPE/SLOT_MAP（:40-50，英雄/物品/通用/隐藏/攻击 槽位分组，SLOT_MAX=1000）。
- 依赖：`require 'base.util'`（:9，桩）。
- 补丁相关：无。

## common/base/buff.lua
- 用途：Buff 类（286 行）——剩余时间/层数/暂停恢复。
- 导出：`return { Buff = Buff }`（:286-288）。
- 关键：`get_buff_name_by_hash`（:17，`common.string_hash`）；`get_remaining`（:45，基于 base.clock）/set_remaining（:36）。
- 补丁相关：无。

## common/base/item.lua
- 用途：客户端物品 Item 类（268 行）——物品本质是带 sys_item_* 属性的单位。
- 导出：`return { Item = Item }`（:268-270）。
- 关键：`mt:__index`（:28-60）把 link/mods/rnds/stack/quality/owner_id/inv_index/slot 等映射到底层 unit 的 `_attribute.sys_item_*`；`cache` 走 `base.eff.cache(link)`。
- 补丁相关：无。

## common/base/actor.lua
- 用途：表现层 Actor 类（1125 行）——客户端特效/模型/声音 C++ API 封装。
- 导出：`return { Actor = Actor }`（:1125-1127）。
- 关键：`actor_map`（弱 :22）/`sid_map`（强 :25，服务端 actor 映射）；`base.set_actor_map`（:27）、`base.set_actor_mode(allow_ray_cast)`（:33）、`base.set_unit_highlight_on/off`（:44）、`base.actor(name, sid, ...)`、`base.actor_info()`（server_actor_map，server.lua:97 消费）、`base.create_actor_at(...)`。
- 补丁相关：服务端可通过 server.lua 的 s2c_rpc 遥控 actor（actor_funcs 白名单 server.lua:109-141）。

## common/base/response.lua
- 用途：响应（Response）类——攻击/受击等效果响应与冷却（474 行）。
- 导出：`return { Response, ResponseDamage, ResponseMissileImpact, ... }`（:474-477）。
- 关键：`base.response` prototype（:19）、`e_location = {Attacker, Defender}`（:22）、`new(link)`（:30）/`set_cache`（:41）/`execute(in_param, ...)`（:53）。
- 补丁相关：无。

## common/base/quest.lua
- 用途：任务/任务条件类（Quest/QuestCondition，443 行）。
- 导出：`return { Quest, QuestCondition }`（:443-446）；`base.quest`/`base.quest_condition`（:8-9）。
- 关键：状态枚举 `active_state`（:17）、`complete_state`（:22）；`base.print_table`（:28，调试工具）。
- 补丁相关：无。

## common/base/cmd_result.lua
- 用途：命令结果 CmdResult 类（e_cmd 包装，支持比较元方法）。
- 导出：`return { CmdResult = CmdResult }`（:61-63）；`base.cmd_result`（:10）。`new()`（:18）、`get_value`（:47）、`get_text`（:54，取 base.eff.e_cmd_str）。
- 补丁相关：无。

## common/base/target_filter.lua
- 用途：目标过滤器 TargetFilters——「需要;排除」字符串解析与校验。
- 导出：无 return；`base.target_filters`（:6）。filters 枚举（:16-46，自身/同一玩家/盟友/中立/敌方/可见/镜像/无敌/魔免/物免/缴械/定身/免死/失控/蝗虫/召唤/死亡/单位/英雄/小兵/首领/建筑/防御塔/基地/图腾/物品/弹道）。
- 关键：`new(filter_string)`（:53，`required,excluded` 以 `;` 分隔、`,` 分项）、`validate(caster,target)`（:103）、`has_filter`（:194，自定义状态走 `sys_unit_mark_ex` 位掩码 :196）。
- 补丁相关：无。

## common/base/单位组.lua
- 用途：单位组（可 +/- 元方法的对象集合，268+ 行）。
- 导出：无 return；`base.单位组(单位数组)`（:268）、`base.create_unit_group(units)`（:277）、`base.unit_group_random_unit(ug)`（:318）、`base.unit_group_random_units(ug, cnt)`（:328）、`base.unit_group_forEachEx(ug, callbackfn)`（:346）。
- 关键：通用 items_table 元表工厂（:2-59，`__add/__sub/__eq/__tostring/add_item`），弱键注册表（:1）；监听 `单位-移除` 自动剔除（:297）。
- 补丁相关：无。

## common/base/snapshot.lua
- 用途：目标快照 Snapshot 类（Target 的静态副本，用于技能参数固化）。
- 导出：`return { SnapShot = Snapshot }`（:122-124，注意键名大小写 S）；`base.snapshot`（:11）。
- 关键：`new/get_snapshot/get_point/get_unit(nil)/get_name/get_owner/get_facing/is_ally/is_visible_to/has_restriction/has_label/get_attackable_radius`（:17-120）。
- 补丁相关：无。

## common/base/slot.lua
- 用途：物品栏位 Slot 类（Excluded/Required 物品分类限制）。
- 导出：`return { Slot = Slot }`（:66-68）；`base.create_slot()`（:28）。
- 补丁相关：无（:62 `test_1` 是残留测试函数）。

## common/base/riseletter.lua
- 用途：飘字 Riseletter 类。
- 导出：无 return；全局 `Riseletter`（:3）、`base.riseletter` prototype（:6）。`new(unit,id)`/`get_id/get_unit/remove/set_screen_position/set_world_position/set_unit`（:12-43，均调 C++ `base.set_riseletter_*`）。
- 补丁相关：无。

## common/base/anim_handlers.lua
- 用途：动画句柄——单次 anim 与三段（birth/stand/death）bracket_anim 的注册管理。
- 导出：无 return。`base.anim(anim_name, owner_type, owner_id, owner_name, params)`（:25）、`base.bracket_anim(anim_birth, anim_stand, anim_death, params, owner_type, owner_id, owner_name)`（:47）、`base.get_anim_map()`（:17）、`base.get_anim_bracket_map()`（:21）；两 map 均弱值（:12、:15）。
- 补丁相关：无。

## common/base/behavior.lua
- 用途：交互行为——鼠标悬停高亮/光标形态、右键点击派单（PCRightButtonActor）。
- 导出：无 return。
- 依赖：`base.eff.cache('$$.gameplay.dflt.root')` 的 HighlightConfig/CursorConfig/PCRightButtonActor（:52-53、:101）。
- 补丁相关：`base.proto.unit_get_interaction_spell`/`unit_remove_interaction_spell`（:1/:12）是服务端推送交互技能的 proto；监听 `鼠标-移动`（:55）/`鼠标-按下`（:103），右键点单位发 `__use_skill`（:117）。

## common/base/camera.lua
- 用途：镜头 Camera 类——当前镜头单例 `base.camera()`，属性经 __newindex 直写引擎。
- 导出：`return { Camera = Camera }`（:209-211）；`base.camera()`（:199）、`base.get_camera_link()`（:80）。
- 关键：attr_key 默认属性表（:12-27，focus_unit_moving_speed/filed_of_view/near_clip/far_clip 等）；`mt.__newindex`（:35）对当前镜头直接 `game.set_camera_attribute`；`rotate_camera/shake_camera/set_camera/set_camera_attribute_number/switch_camera/get_position/get_rotation/get_distance/set_as_active`（:107-197）。初始化等 `Src-PostCacheInit`（:89-95）。
- 补丁相关：`base.proto.set_camera`（:203）服务端切镜头。

## common/base/screen.lua
- 用途：屏幕/分辨率/安全区封装 `base.screen`。
- 导出：无 return。`get_orientation/get_resolution/set_resolution(w,h)`（:7-24，移动端走 set_logic_view）、`get_bangs_height`、`input_mouse(touch_id?)`（:31）、`set_cursor_visible`、`get_safe_insets()`（:49，wx/qq 走 base.wx.call，移动走 common.get_safe_area_insets）、`enable_safe_area(enable)`（:73）。
- 依赖：`include 'base.platform'`（:1，桩）。
- 补丁相关：引擎事件 `on_screen_resolution_changed`（:85）/`on_orientation_changed`（:90，含分辨率自动纠正逻辑）。

## common/base/terrain.lua
- 用途：地形贴图查询 `base.terrain`（get_texture_name/tag/info，:3-15，均调 C++ `game.get_texture`）。
- 导出：无 return。
- 补丁相关：无。

## common/base/scene.lua
- 用途：多场景管理——场景事件按场景订阅/退订、场景激活状态。
- 导出：`return { set_scene_activated, set_scene_not_activated, is_scene_activated, get_activated_scenes, get_obj_scene_events }`（:149-155）。
- 依赖：`include 'base.server'`（:1）。
- 补丁相关：`base._scene`（:3）存各场景对象事件表；`base.proto.__server_jump_scene`（:143）服务端切场景通知；被 trigger.lua:10 require。

## common/base/shortcut.lua
- 用途：快捷键注册封装 `base.shortcut`（仅 StateGame，:1-3 其他 state 直接 return）。
- 导出：无 return。`register(name, func)`（:11）、`has_registered/unregister/get_shortcut_pressed/lock/unlock/lock_all/unlock_all`（:22-43，均调 C++ `shortcut.*`）。
- 补丁相关：全局 `_G.shortcut_events`（:5）+ `shortcut_events.on_shortcut_pressed`（:16）是 C++ 回调入口。

## common/base/settings.lua
- 用途：游戏设置项 `base.settings`——get/save/set/register option（对 C++ `common.*_option` 的类型分派封装）。
- 导出：无 return。`get_option`（:6）、`save_global_option/save_option/set_option/set_default_option`（:12-50）、`register_option(name, func)`（:52）、`set_current_game/get_current_game`（:57/:64）。
- 补丁相关：`base.event.on_settings_changed`（:68）是 C++ 设置变更回调；:74-78 `save_replay` 特判（StateGame 下卸载地图时复位）。

## common/base/startup.lua
- 用途：装备图（map_kind==2）进入前台流程——启动弹窗模块注册与顺序执行。
- 导出：无 return。`base.startup.register_pre_enter_foreground_callback(cb)`（:8）、`register_startup_function(check_is_startup, startup_dialog)`（:12）。
- 补丁相关：监听 `游戏即将进入前台`（:72，多回调 confirm/cancel 计数汇合）与 `游戏进入前台`（:100）；读 `common.get_value('Equipment_Game_Mode')`（:48）；`startup_dialog` 链尾 `lobby.return_to_lobby()`（:33）。

## common/base/open_url_wrap.lua
- 用途：**包装 `common.open_url`**（:91）——URL 白名单校验（StateGame 下仅白名单放行，:71-88 经 sce.s.score_init 拉白名单，:93-104 内置 QQ 系前缀），`start-game://` 协议转 `switch_game` 广播（:107-109）。
- 导出：无 return。
- 依赖：`require '@common.base.gui.component'`（:2）、`require 'base.argv'`（:44）、`require 'base.lobby'`（:69、:92）。
- 补丁相关：**这是对 C++ 全局函数的覆盖点**——补丁可参照此模式包装其他 `common.*` 函数；`editor_server_debug` argv 下不拉白名单（:71）。

## common/base/game_result.lua
- 用途：默认结算界面（胜负特效 actor + 「游戏结束」面板）与退出/再来一局 proto。
- 导出：无 return。`base.proto.default_game_result`（:88）、`lobby_game_exit`（:108）、`__one_more_round`（:115）、`base.game.one_more_round()`（:119）。
- 依赖：base.ui（面板 :1-62）、`require '@common.base.lobby'`（:72）。
- 补丁相关：仅 StateGame 加载（init.lua:164）。

## common/base/select_indicator.lua
- 用途：选中指示器——监听 `单位-选中/取消选中`，按 gameplay.SelectIndicator 配置贴特效。
- 导出：无 return；`base.select_indicator`（:1，当前 actor 引用）、开关 `base.select_indicator_enable`（:4）。
- 补丁相关：无。

## common/base/select_hero.lua
- 用途：选英雄流程 `base.select_hero`——可选列表/点击/随机/倒计时。
- 导出：无 return。`hero_list()`（:8）、`select_hero(name)`（:12，`game.request_pick_hero`）、`click_hero`（:20）、`click_random_hero`（:28）、`show_timer`（:32）、`show_hero`（:39，已注释空转）、`show_random`（:48）。
- 补丁相关：引擎回调 `base.event.on_hero_pick_start_notify`（:53）等 4 个（:82/:89/:96）。

## common/base/shell.lua
- 用途：**远程 shell——`base.proto.__shell`**（:2-8）：服务端可下发任意 Lua 代码串，客户端 `load(info.code)` 后 pcall 执行并回传结果。
- 导出：无 return。
- 依赖：`base.game:server '__shell'` 回包。
- 补丁相关：**高价值**——官方预留的服务端→客户端任意代码执行通道，仅非 app 平台加载（init.lua:152-155）。isolation 后 load 被强制 mode='t' 但仍可执行文本代码。

## common/base/debugger.lua
- 用途：返回一个启动调试器的函数（Windows 上限定）。
- 导出：`return function(wait)`（:1-7）——`require 'debugger'`（外部模块），`dbg:io('listen:0.0.0.0:4278')`，wait 为真则阻塞等调试器附加，`dbg:start()`。
- 依赖：`common.get_platform()`（:2）。
- 补丁相关：**官方远程调试入口（端口 4278）**，补丁可直接 `require 'base.debugger'` 拿到此函数启用 luadebug。

## common/base/error_info.lua
- 用途：`base.get_error_info()`（:3-10）——读 `__MAP_NAME` 文件拿版本号，返回 `{map_name, version}`。
- 导出：无 return。
- 补丁相关：捕获 `io.read` 到 local（:1，在 isolation 前 include 加载，init.lua:148，拿到的可能是未阉割版 io.read——推断）。

## common/base/output.lua
- 用途：调试输出面板（____output UI），`info/error` 两个写日志到界面。
- 导出：`return { info, error }`（:46-49）。
- 依赖：`base.ui.panel/label/create`。
- 补丁相关：无。

## common/base/console.lua（桩）
- `return require '@base.base.console'`（:1，注释「代码迁移到client_base」）。注意 common/main.lua:15 以 `include 'base.console'` 加载——**include 每次执行都转发到 client_base 的 console**，控制台实现不在镜像。

## common/base/cheat.lua
- 用途：作弊码/GM 命令体系（22KB）。前半（:8-110）是被整体注释掉的旧 reload/include 机制（历史参考：曾用 `argv.has('debug')` 决定 include 走热更还是 require）。
- 导出：无 return。`base.cheat = {}`（:6）。
- 关键：监听 `玩家-输入作弊码`（:113-126）分派 `gm[name](cmd)`；内置 `gm.showmovejoystick/smj`（:129-141）；`base.proto.__gm_debug_unit`（:174）/`__gm_debug_player`（:186）等 GM 调试面板 proto。
- 依赖：`require '@common.base.platform'`、`require '@common.base.argv'`、`require '@common.base.gui.component'`（:1-3）。
- 补丁相关：**加自定义作弊码只需 `gm` 表扩展（推断：gm 是 local，外部无法直接扩；需改本文件或 hook `玩家-输入作弊码` 事件抢先处理）**。

## common/base/state_machine.lua
- 用途：自定义状态机——包装 SCE.StateMachine/SCE.SMState（C++ 触编上下文）。
- 导出：无 return。`base.state_machine(name, priority?, layer?)`（:21）、`base.state_machine_state(name, id)`（:32）。
- 补丁相关：**:1 守卫 `if not (ImportSCEContext and __lua_state_name == 'StateGame') then return end`**——仅游戏 state 生效；`ImportSCEContext()`（:2）是 C++ 注入的 SCE 上下文获取函数。

## common/base/tracer.lua
- 用途：调用树耗时追踪器（debug.sethook 'cr' 实现）。
- 导出：`return tracer`（:136）；`tracer.new{depth={limit=100}, filter}`（:20），方法 `start()`（:121，`debug.sethook(proxy,'cr')`）、`finish()`（:127）、`output()`（:60，打 log_file+print）、`pause/resume`。
- 依赖：`include 'base.profiler'`（:2）。
- 补丁相关：**StateGame 下 debug.sethook 已被 isolation 置 nil，本模块只能在 StateEditor/StateApplication 或未阉割窗口用**（与 isolation.lua:219 对照）。

## common/base/profiler.lua
- 用途：简易耗时统计器（`common.get_system_time` 毫秒）。
- 导出：`return profiler`（:45）；`profiler.new()`（:9）→ `:start()/:finish()/:get_used()/:get_elapse()/:reset()`（:19-43）。
- 补丁相关：被 timer/server/tracer 复用。

## common/base/mprofiler_text.lua
- 用途：内存 profile 报告文本化（flat/call/all 三种视图）。
- 导出：`return mprofiler_text`（:72）；`mprofiler_text(call_table, prof_type, size)`（:24）。
- 依赖：`require "base.mprofiler_reduce"`（:1）、`require "base.mprofiler_ascii_print"`（:2）。
- 补丁相关：无（mprofiler 四件套：lt2oa/reduce/ascii_print/text，lmprof 移植）。

## common/base/mprofiler_reduce.lua
- 用途：lmprof 调用表归约成函数图。
- 导出：`return reduce`（:80）；`reduce(call_table) -> func_table, func_graph`（:16）。
- 补丁相关：无。

## common/base/mprofiler_lt2oa.lua
- 用途：lmprof 表转排序数组（按 mem_self/mem_cum/calls 排序）。
- 导出：`return lmprof_table2orderd_array`（:30）。
- 补丁相关：无。

## common/base/mprofiler_ascii_print.lua
- 用途：lmprof 报告 ASCII 表格打印（flat_print/call_graph_print）。
- 导出：`return { flat_print, call_graph_print }`（:181-184）。
- 依赖：`require "base.mprofiler_lt2oa"`（:1）。
- 补丁相关：无。

## common/base/lni_writer.lua
- 用途：table → lni 文本序列化器（`[root]` 节，键值转义）。
- 导出：`return function(lni) -> string`（:57-65）。
- 补丁相关：server.lua:201 收到未知消息时用它美化日志。

## common/base/table_writer.lua（桩）
- `return require "@base.base.table_writer"`（:1）。实现（base.table_unpack 等）在 client_base。

## common/base/ad.lua
- 用途：激励视频广告封装 `show_reward_video_ad`，失败时回退自定义广告（webview 播 MP4）。
- 导出：`return { show_reward_video_ad = show_reward_video_ad }`（:60-62）；同时 `rpc.show_reward_video_ad`（:58）。
- 依赖：`require 'base.server'`、`require 'base.rpc'`、`include('base.sdk').on_json`、`include "base.co"`、`require 'base.argv'`、`require 'base.gui.component'`（:2-7）、`base.calc_http_server_address`（:9）。
- 补丁相关：无。

## common/base/voice.lua
- 用途：语音房间封装（进出/黑名单/说话状态事件）。
- 导出：`return sdk`（:107）。注册 `rpc.join_voice_room(room, team, range, cb)`（:16）、`rpc.voice_black_list(p, mute)`（:44）。
- 依赖：`require 'base.rpc'`、`include('base.sdk').on_json`、`include "base.co"`、`require 'base.argv'`（:2-5）；仅当 C++ `sdk.exit_voice_room`/`join_voice_room` 存在时启用（:9）。
- 补丁相关：事件 `语音-开启/进入/退出/开始说话/停止说话`（:36、:64-66、:79、:87）。

## common/base/lobby.lua
- 用途：大厅模块壳——转发 client_base lobby 并追加 `lobby.app_lua.play_custom_ad`（自定义广告全屏 webview 面板实现，:6-152）。
- 导出：`return lobby`（:154）。
- 依赖：`require '@base.base.lobby'`（:2）、`require 'base.argv'`、`include 'base.co'`（:3-4）。
- 补丁相关：`lobby.vm_name()`/`lobby.send_luastate_broadcast`/`lobby.dispatch_all_vm`/`lobby.return_to_lobby` 等被全库广泛使用，但实现在 client_base（推断：跨 Lua state 广播机制）。

## common/base/friend.lua
- 用途：局内好友——申请/同意/拒绝发送 + 好友列表 proto 转事件。
- 导出：无 return；`base.friend.send_add_friend/agree/refuse(user_id)`（:3-20）；proto `InGame_S2C_init_friend_list` 等 4 个（:24-45）转 `好友-*` 事件。
- 补丁相关：无。

## common/base/spell_assist_control.lua
- 用途：技能施法辅助指示器控制（60KB，最大文件之一）——鼠标/摇杆两种操作方式，圆/矩形指示器。
- 导出：`return { OPT_MOUSE, OPT_JOYSTICK, ... }`（:1220-1223，含切换操作方式等接口）。
- 关键：枚举 OPT（:8）、MT（:14）、ST（:20）、SI（:26）、STICKING（:33）。
- 依赖：`include 'base.platform'`（:1）、`require 'base.util'`（:5）。
- 补丁相关：被 game.lua 的 `on_control_spell_assist`（game.lua:474）等引擎事件驱动。

## common/base/lualib_bundle.lua
- 用途：**TypeScriptToLua 运行时库**（2854 行，95KB）——`__TS__Class/__TS__ClassExtends/__TS__Array*`/`__TS__Promise` 等全套 TS 编译产物支撑函数。
- 导出：`return { __TS__ArrayClone, __TS__ArrayConcat, __TS__ArrayEntries, ... }`（:2854 起全量 __TS__* 表）。
- 补丁相关：赋给 `base.tsc`（init.lua:68），全库类体系（`base.tsc.__TS__Class()`）的地基；触编生成的 TS 事件类（event.lua:362 起）同样依赖。纯运行时，无业务逻辑。

## common/base/obj_check.lua
- 用途：参数类型校验函数群（全局函数，非 base 表）+ UI 淡入淡出。
- 导出：无 return；全局 `unit_check/item_check/skill_check/player_check/circle_check/rect_check/area_check/point_check/line_check/buff_check/trigger_check/timer_check/any_unit_check/any_skill_check/any_player_check/id_check/event_name_check/time_check/component_check`（:1-220，均带 disable_error 参数，失败 log.error 并返回 false）。
- 另：`base.gui_check/gui_get_part_as/gui_get_parts_ts/gui_get_array_child/gui_get_child_ui_by_name_as/gui_get_children/gui_get_rect/gui_get_parent`（:222-282）；`base.fade_in_out/fade_in/fade_out`（:286-366，fade_panel 组件 + coroutine.sleep 等待）。
- 依赖：`require '@common.base.gui.control_util'`（:211）。
- 补丁相关：无。

---

## 转发桩条目（统一格式）

以下 31 个文件本体只有一行 `return require '@base.base.<同名>'`（实现均在 client_base 库，不在本镜像；个别带 GBK 注释「代码迁移到client_base/迁移到base」）：

## common/base/argv.lua
- 用途：命令行参数查询（`argv.has(name)`/`argv.get(name)`，由全库调用点归纳）。
- 导出：`return require "@base.base.argv"`（:2）。
- 依赖：@base.base.argv（跨库 client_base）。
- 补丁相关：**isolation.lua、log.lua、timer.lua、cheat.lua、open_url_wrap.lua 的行为开关全经它读取**（`editor_server_debug`/`debug`/`lua_debug`/`inner`/`test`/`unit_test`/`auto_test`/`game`/`portrait` 等键），但实现不可考。命令行参数由引擎进程启动参数注入（推断）。

## common/base/path.lua
- 导出：`return require "@base.base.path"`（:1）。被 isolation.lua:10 用于拼接地图目录路径。实现在 client_base。

## common/base/util.lua
- 导出：`return require "@base.base.util"`（:1）。被 skill.lua/spell_assist_control.lua 等 require；common/main.lua:32 以 `@base.base.util` 直接引用 client_base 版。实现在 client_base。

## common/base/platform.lua
- 导出：`return require '@base.base.platform'`（:1）。`is_app()/is_mobile()/is_wx()/is_qq()` 等判定（由 init.lua:101、screen.lua:17、cheat.lua:128 调用点归纳）。实现在 client_base。

## common/base/co.lua
- 导出：`return require "@base.base.co"`（:1）。协程工具（`co.wrap/co.async/co.sleep`，由 promise.lua:23-24、lobby.lua:137-139、voice.lua:21 调用点归纳）。**注意 `coroutine.async`/`coroutine.sleep`/`coroutine.promise` 等扩展方法也是它挂到标准 coroutine 表上的**（推断，由 voice.lua:18、obj_check.lua:343、promise.lua:223-229 用法归纳）。实现在 client_base。

## common/base/json.lua
- 导出：`return require '@base.base.json'`（:1）。`base.json.encode/decode` 与全局 `json`（friend.lua:25、select_hero.lua:54 用法）。实现在 client_base。

## common/base/class.lua
- 导出：`return require "@base.base.class"`（:1）。OOP class 函数（state_machine.lua:4 `class('CustomStateMachine', SCE.StateMachine)` 用法）。实现在 client_base。

## common/base/try.lua
- 导出：`return require "@base.base.try"`（:1）。异常捕获工具。实现在 client_base。

## common/base/exception.lua
- 导出：`return require "@base.base.exception"`（:1）。`to_exception`（promise.lua:25 用法）。实现在 client_base。

## common/base/web.lua
- 导出：`return require "@base.base.web"`（:1）。web 平台适配。实现在 client_base。

## common/base/sdk.lua
- 导出：`return require '@base.base.sdk'`（:1）。C++ `sdk` 全局的 Lua 增强（`on_json` 事件订阅，voice/ad 均 `include('base.sdk').on_json`）。实现在 client_base。

## common/base/ip.lua
- 导出：`return require "@base.base.ip"`（:1）。IP/地址工具。实现在 client_base。

## common/base/toast.lua
- 导出：`return require "@base.base.toast"`（:1）。飘字提示（friend.lua:6 全局 `toast(...)` 用法）。实现在 client_base。

## common/base/deque.lua
- 导出：`return require '@base.base.deque'`（:1）。双端队列。实现在 client_base。

## common/base/event_deque.lua
- 导出：`return require '@base.base.event_deque'`（:1）。`create_event_queue`（promise.lua:22 用法：`:pop(timeout, cb)`/`:close()`）。实现在 client_base。

## common/base/request.lua
- 导出：`return require "@base.base.request"`（:1）。实现在 client_base。

## common/base/confirm.lua
- 导出：`return require '@base.base.confirm'`（:1）。确认对话框。实现在 client_base。

## common/base/account.lua
- 导出：`return require "@base.base.account"`（:1）。`latest_login_info.user_id`（voice.lua:25 用法）。实现在 client_base。

## common/base/progress.lua
- 导出：`return require "@base.base.progress"`（:1）。进度条。实现在 client_base。

## common/base/check_log.lua
- 导出：`return require '@base.base.check_log'`（:1）。实现在 client_base。

## common/base/file_mutex.lua
- 导出：`return require "@base.base.file_mutex"`（:1）。文件互斥锁。实现在 client_base。

## common/base/disconnect.lua
- 导出：`return require "@base.base.disconnect"`（:1）。断线处理（init.lua:160 已注释掉加载）。实现在 client_base。

## common/base/localization.lua
- 导出：`return require '@base.base.localization'`（:1）。`base.i18n.get_text`（friend.lua:6、open_url_wrap.lua:59 用法）。实现在 client_base。注意与 common 根级 localization.lua（有实实现）是两个文件。

## common/base/upload_log.lua
- 导出：`return require '@base.base.upload_log'`（:1，注释「迁移到base」）。日志上传。实现在 client_base。

## common/base/wx.lua
- 导出：`return require '@base.base.wx'`（:1，注释「代码迁移到client_base」）。`base.wx.call('get_system_info')`（screen.lua:52 用法）。实现在 client_base。

## common/base/replay.lua
- 导出：`return require '@base.base.replay'`（:1，注释「代码迁移到client_base」）。录像。实现在 client_base。

## common/base/json_load.lua
- 导出：`return require '@base.base.json_load'`（:1）。实现在 client_base。

## common/base/json_save.lua
- 导出：`return require '@base.base.json_save'`（:1）。实现在 client_base。

## common/base/anim.lua
- 导出：`anim = require '@base.base.anim'; return anim`（:1-2）——**额外把结果写入全局 `anim`**（其他桩不写全局）。实现在 client_base。

## common/base/margin.lua
- 用途：客户端空实现占位（注释「逻辑全在服务端，客户端只需要空函数」:1）。
- 导出：无 return；`function base.margin(...) end`（:2-3，空函数）。
- 依赖：无（直接写 base.margin，依赖加载顺序）。
- 补丁相关：无。
