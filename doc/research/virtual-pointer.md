# 0.8.2 虚拟指针机制与实测结论（R0 研究 + 实现终验）

> 2026-08-29。R0 四问实测见下文；实现完成后的真机终验结论追加在文末「实现终验」。
> 需求背景见 doc/requirements/0.8.2.md。

## 前置基建：lua.eval（本期新增）

桥侧此前只有编辑器 VM 的 `lua.run_lua`，游戏 VM 无脚本执行通道，R0 探测做不了。
本期补：`ui_loop.lua` 加 `handlers.eval` 薄转发（dbg_bus 游戏侧本就有 `commands.eval`），
catalog 静态条目 `lua.eval`（danger 级，与 run_lua 同级，进审计日志）。
**这是常驻能力**（游戏侧逃生舱），非一次性探针。

## Q1 键盘模拟能力面 —— 通过，进本期

实测序列（lua.eval 注入临时监听 + 调引擎转发 API）：

```lua
base.event_register(base.game, '按键-按下', function(_t, key) ...记录... end)
base.event_register(base.game, '按键-松开', function(_t, key) ...记录... end)
base.event.on_key_down('W')  -- 监听立刻收到 'W'
base.event.on_key_up('W')    -- 监听立刻收到 'W'
```

- `base.event.on_key_down(unkey)` / `on_key_up(unkey)` **脚本直调有效**，合成的按键事件
  与真实物理按键走同一分发（'按键-按下'/'按键-松开' 游戏事件）。
- 按住 = down 后不 up（实测 down 后 600ms 内无 up 事件，调 on_key_up 才收到）——三态齐全。
- 键名取字符串（'W'），与 `bgd_const.keyboard` 常量同源。
- **结论**：本期新增 `lua.key_down {key}` / `lua.key_up {key}`（游戏侧 dbg_commands 一行包装，
  桥薄转发，catalog write 级）。文本输入仍走 input_text（on_input 字段直写），不逐键模拟。

## Q2 on_drop 跨帧数据载荷 —— 机制成立（代码实证 + 注入先例），实测并入 R2.3 验收

- 拖拽源（kit.lua drag_ghost, L623-628）**每帧**把业务 data 写进自身 `imgui.data()` 持久表
  （跨帧存活），不是只在 drag_begin 瞬间存在——捕获窗口充裕。
- `dbg.on_frame_state(frame)` 钩子的调用点（core.frame_state, core.lua:542-553）在被钩控件的
  begin/end 上下文内（栈顶即该帧）——**钩子里 `imgui.data()` 取到的就是该控件的持久数据表**，
  drag_ui 可在源控件 down 帧顺手抓取载荷。
- 放置目标（control.lua drop_target L725 / sortable_list L137）读 `st.on_drop` 表——
  与已验证的 `on_real_click` 注入同属「克隆 st 覆写字段」机制，注入载荷表即可触发业务 on_drop。
- 落点高亮 `st.drop_hover` 同理可注入。
- **结论**：drag_ui 可行，不需要降级为「拖放组件注册回调特化」。bench（wc drag_drop /
  sortable_list）终验放在 R2.3 实现后（chicken-egg：没有 drag_ui 无法脚本化拖放）。

## Q3 pscroll 注册通道 —— 定为「pscroll 挂句柄进 dbg 条目」

- pscroll 的 scroll_to 句柄是每帧新建闭包，但闭包捕获的 `st`（remember_slot 'pscroll'）
  跨帧持久——句柄闭包存进 dbg 快照条目后跨帧调用仍有效。
- 现状 pscroll 完全无 dbg 注册；scroll_ui 需要从 dbg 侧拿到 scroll_to/offset。
- **结论**：pscroll 在 `core.dbg.enabled` 时把 `scroll_to`/`offset` 闭包挂进 dbg 条目
  （框架内部机制，pscroll 是框架代码——不属于 widget 作者契约，与 R1 删 dbg_register 不冲突）。
  引擎 scroll 容器（cg.scroll/vlist）不强行驱动：scroll_ui 命中非 pscroll 容器时返回
  actionable 错误（建议业务迁 pscroll）。

## Q4 命中测试 clip 边界 —— v1 简化（rect 命中）够用

实测：bench 内置组件页详情面板（滚动区）中 `listen_on_drop`（y=1510）已超出逻辑屏高
（1463.75），**仍在快照中且 rect 为真实布局坐标**——被裁剪/屏外子控件 rect 不丢、坐标真实。
由此：

- 屏外条目的 rect 自然落在视口外，按点命中不会误选（点在屏内，rect 在屏外，不相交）；
- 部分可见条目（pscroll 内容上移后 y<0 的条目）rect 同样是真实坐标，点其可见部分命中正确；
- 残余边缘场景（rect 与视口相交但被更小的 clip 区裁掉的部件）v1 接受误差，v2 再算父链 clip 交集。
- **结论**：R2.2 v1 = 快照 rect 倒序命中（绘制序近似 z 序）+ 可见性，不做 clip 交集。

---

## 实现终验（2026-08-29 真机，全部经 lua.eval/新命令在 test_res002 PIE 实证）

### 机制定稿（与 R0 推论一致 + 三处实测修正）

- **vp 状态机**在 dbg.lua：`{x, y, down}` 逻辑坐标态 + 步态序列（vp_program，帧首消费）
  + 保持态（vp_hold 按住 / vp 驻留 hover）；注入兑现点 = on_frame_state 钩子克隆 st 覆写。
- **指针覆写**：core.frame_reset 在 vp 激活时以 vp 覆写指针缓存；widget 层
  `base.screen:input_mouse()` 直读全部收敛到 `core.pointer()`（slider/joystick/kit/
  container/sortable_list；page_diag 是刻意的引擎原值探针，保留）。
- **实测修正 1（命中测试）**：finish 顺序里容器晚于子控件 → 直接倒序会被父容器
  截胡（pscroll 静态 content 面板盖住下方全部条目）。定稿 = static 控件不标
  interactive + 剔除「是其他候选祖先」的容器 + 剩余按 order 倒序。
- **实测修正 2（拖放落点）**：命中件可能是放置目标的子件（sortable 行的 drag 子件
  挡住 drop）——落点解析上溯最近的 `enable_drop` 祖先（core 在 begin 时记录
  frame.drop，快照条目带 drop 标记）。
- **实测修正 3（按钮长按潜伏 bug）**：button_impl 的 `interact_dispatch(st, o)` 的 st
  是块内 local（越界 nil），on_long_press 从未真实触发——已修复（st 提升到 begin
  作用域），vp 长按实测 800ms 处触发 on_long_press。

### 逐命令真机结果

| 命令 | 证据 |
| --- | --- |
| tap | hub 入口/cgui_bench/游戏件菜单连点全绿（文本命中叶子→祖先） |
| click_ui/click_at | 注入统一 state_inject；popup 点遮罩关闭（click_at 150,1300 命中 mask） |
| input_text | bi_color 输入 #FF8800FF 下帧生效（st.on_input 注入） |
| press_ui/release_ui | kit_joy 按住 vec=(1,0)/(0,-1) len=1.00「按住中」→ 松开「已松开」 |
| set_value | kit_segs_sl 设 3 → actual=3（vp 点击轨道 + on_real_click 提交 on_commit） |
| hover_ui | kit_tip_cell1 保持态 → tooltip overlay 开出（精铁剑/攻击+120 入快照） |
| drag_ui | sortable row5→row1：载荷 {key,index,_sortable} 捕获，on_move(row5,row1)，视觉序重排 |
| scroll_ui | kit_pscroll delta_y=120/200 → offset 精确变化 |
| pick | kit_quality 展开→选项「高」点击→菜单关闭（业务 dummy state 故选中值不变，机制已对） |
| long_press_ui | kit_gesture_btn 按住 800ms 处 on_long_press 触发（松手后 on_click 覆盖显示，符合真实语义） |
| key_down/key_up | U 关商店 / Y 开背包（游戏面板验收场景实证） |

### 场景脚本（R3）与回归

- save_as/{$名}/wait_for/assert_text 单测 + 真机链路全绿（变量默认取
  clickable_ancestor/id，find→click 串联实证）。
- **bench_sweep 重写**：0.8.0 的 150 行 ps1 逐步往返 → 0.8.2 场景 71 步一次调用
  （test/0.8.2/case/bench_sweep082.ps1），全绿 + 日志 errors=0。
- **存量界面逐面验收**（test/0.8.2/case/game_panels082.ps1，34 步全绿 + errors=0）：
  商店（cgui ShopUI：tab 切换+断言+U 键关闭）/ 背包（cgui BagUI：Y 键开+整理+X 关）/
  GM（**base.ui 旧体系，find_ui 只定位不可操作——R4 边界确认，非回归**）/
  HUD（hud_shop 商店入口 = 2D 场景 cgui 覆盖件，已操作生效）/ 调试台（sweep 全覆盖）。

### 遗留边界（与 cgui_mcp.md §5 同步）

- sortable_list 拖拽中的边缘自动滚动依赖引擎 view_state().drag，vp 不经过——
  v1 接受（目标行可先 scroll_ui 滚进视口）。
- 引擎 scroll 容器（cg.scroll/vlist）不支持程序滚动——scroll_ui 报 actionable 错误。
- 世界拾取（点地面/选怪）仍是非 cgui 场景层交互，列开放研究项。
