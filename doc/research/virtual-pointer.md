# 0.8.2 R0 前期研究结论（虚拟指针 / 键盘模拟 / 拖放载荷 / clip 命中）

> 2026-08-29 上机实测（test_res002 PIE，StateGame VM，经新增 `lua.eval` 游戏侧逃生舱）。
> 需求背景见 doc/requirements/0.8.2.md「R0 前期研究」。本期虚拟指针完整机制结论在实现后增补进本文或 ui-reflection.md。

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
