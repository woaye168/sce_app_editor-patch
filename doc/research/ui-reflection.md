# UI 反射机制研究（0.8.0 R0 结论，全部真机验证）

> 日期：2026-08-29 ｜ 验证环境：api13（D:\sce_online\version-13）+ test_res002 PIE
> 关联：[lua-vm-bus.md](lua-vm-bus.md)（跨 VM 总线细节）、需求 `doc/requirements/0.8.0.md`

## 一句话结论

游戏 UI 闭环调试落地形态 = **lobby 跨 VM 总线（编辑器→游戏命令通道）+ cgui 调试反射（快照/回调注册/state 注入）**，
引擎层不做任何输入注入（脚本层做不到，见 Q2）。

## Q1：PIE 下 VM 与 lobby 总线（实锤）

- PIE 调试时游戏 client 跑在 **StateGame**（与编辑器 StateEditor 同进程不同 VM；服务端默认在云端调试 host）。
- 编辑器侧（xdeditor，StateEditor）`require '@base.base.lobby'` 可用，`vm_name()='StateEditor'`，
  `send_luastate_broadcast` / `register_luaState_event` 均在。
- 游戏侧（StateGame）用 `require '@common.base.lobby'`（`@base.base.lobby` 亦可，dbg_bus 双路径兜底）。
- **双向实测通过**：编辑器 `send_luastate_broadcast('bgd_dbg_cmd', ...)` → 游戏注册接收 →
  游戏回 `bgd_dbg_result` → 编辑器接收。链路日志：`base/base/lobby.lua:353 on_luastate_notify, key[bgd_dbg_cmd]`。
- 注意：`__lua_state_name` 全局在两个 VM 实测均为 nil（知识库此前描述有偏差），判 VM 用 `lobby.vm_name()`。

## Q2：引擎输入/imgui 可否脚本注入（实锤：不能）

- SCE 的 "imgui" 不是 Dear ImGui：是引擎 C 层注册在全局 `ui` 表的 `ui.imgui_*` 原生函数组，
  appui 包 Lua 层封装为 `require '@appui.imgui'`。**无 io 表 / AddMousePosEvent / set_mouse_pos**。
- 输入是**拉取式**：每帧 `imgui.state()` 返回 hover/on_real_click/on_input 等字段；无写入入口。
- StateGame 的 `ui` 表全量 dump（164 函数）里与输入注入相关的只有：
  - `ui.vk_key_click(vk)`——键盘虚拟键点击（client_base/console.lua 用来模拟 F1~F12），**无坐标、不含鼠标语义**；
  - `ui.GetControlAtPosition(x, y)`——持久控件树命中测试（只读）；
  - `common.get_mouse_screen_pos()` / `set_cursor_shape/visible`——读坐标/光标外观（只读）。
- 结论：R3 走**纯 Lua 反射**（cgui 回调注册 + state 注入），不做 Win32 注入（保持后台可交互、不抢焦点）。

## Q3：find_ui 游戏侧挂点（实锤）

- cgui 即时模式：控件每帧重建（mount.tick → on_post_update 驱动），无持久控件树。
- 挂点 = `core.finish()`（所有控件闭合的唯一汇流点）：复用已缓存的 `top.st`（finish 无条件兜底
  `imgui.state()`），`imgui.ui_get_rect(st.id)` 取 rect。双缓冲：帧首 `frame_reset` 交换，
  查询读「上一完整帧」快照。
- 交互回调由 widget 层 `core.dbg_register(kind, fn)` 注册（button/checkbox/switch/radio/icon_button/input/
  joystick），与 rect 同帧沉淀。
- **手写交互控件**（core.leaf/begin + children 里自读 state，如 bench 菜单 menu_item）不进注册表——
  兜底走 `core.dbg_inject_click(id)`：frame_state 读到待注入 id 时克隆 st 并置 `on_real_click=1`，
  等价真实点击分发路径（真机验证：bench 菜单翻页成功）。
- 惰性开启：`core.dbg_enabled` 默认 false（零开销），首个查询/交互命令开启；PIE 退出随 VM 消亡。
- 快照预热：base.next 与帧首交换次序不确定，固定帧数延迟不可靠——改为「快照非空或 15 帧重试耗尽
  才执行」（真机踩坑后定稿）。

## Q4：文本输入注入点（实锤）

- input 是受控件：显示值来自调用方，引擎经 `state().on_input` 上报新文本，cgui 直接调业务 on_input。
- 注入点 = widget 层注册的 `input` 回调：`input_text(id, text)` 直接调 `on_input(text)`，
  下一帧内容签名变化 → 引擎重应用 → 界面反映（真机验证：bench 样式编辑器颜色框写 #FF8800 生效）。

## 坐标系（真机校准，易踩坑）

- 游戏侧三个空间：引擎分辨率空间（`common.get_resolution()` = `ui_get_rect` 原始返回，本例 1864×1166）
  → cgui 逻辑空间（÷ `base.ui.auto_scale.current_scale()`，本例 2340×1463，**find_ui/click_at/crop 统一用它**）
  → 编辑器 UI 逻辑空间（get_game_view_rect 的视口 rect 所在，本例视口 328,72,1864,1166 / 全窗 2560×1284）。
- 游戏逻辑空间会**随窗口尺寸变化**（auto_scale）——crop/click_at 的坐标可能过期，报错会提示重新 find_ui。
- capture_game crop 换算链：游戏逻辑 →（× rw/gw，gw 来自 lua.game_info）→ 编辑器逻辑 →（× 帧宽/编辑器逻辑宽）
  → 帧像素。真机验证：find_ui 取商店按钮 rect → crop 截出正好是该按钮。

## 能力边界（诚实清单，0.8.2 更新）

| 操作 | 形态 | 状态 |
| --- | --- | --- |
| find_ui（id/文本子串、kind=click/input/scroll 盘点） | cgui 快照 + base.ui 树 | ✅ |
| click_ui / click_at | state 注入（0.8.2 起唯一路径，注册回调已删除） | ✅ |
| tap / pick（复合：文本一步点 / 下拉一步选） | find + 注入 | ✅（0.8.2 新增） |
| input_text | st.on_input 字段注入（0.8.2 起） | ✅ |
| press_ui / release_ui（joystick 持续方向输入） | 虚拟指针按住（0.8.2 起，sim 槽位已删） | ✅ |
| long_press_ui | 虚拟指针按住超时 | ✅（0.8.2 起） |
| set_value（slider） | 按值反算轨道坐标 + 虚拟指针点击（返回实际生效值） | ✅（0.8.2 起） |
| hover/移入 | **0.8.2 起真实悬停**：虚拟指针保持态（enter/leave 沿齐全，tooltip 实测开出） | ✅（0.8.2 解决） |
| drag_ui（拖放/排序/偏移） | 虚拟指针帧序列 + imgui.data() 载荷捕获 + enable_drop 落点解析 | ✅（0.8.2 新增，sortable_list 重排真机过） |
| scroll_ui（pscroll 直驱） | pscroll 挂载 scroll_to 句柄 | ✅（0.8.2 新增；引擎 scroll 容器报 actionable 错误） |
| key_down / key_up（键盘按下/松开/按住） | base.event.on_key_down/up 脚本直调（与物理按键同分发） | ✅（0.8.2 新增，Q1 实测通过） |
| eval（游戏 VM 逃生舱） | dbg_bus commands.eval 经桥薄转发（danger 级） | ✅（0.8.2 新增） |
| base.ui 持久树控件（编辑器侧/游戏侧） | 只定位（rect），不可点击 | 部分（点击无注入通道） |

> 0.8.2 虚拟指针机制与实测结论详见 [virtual-pointer.md](virtual-pointer.md)。
