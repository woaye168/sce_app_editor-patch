# Lua 多 VM 与 lobby 跨 VM 消息总线（0.8.0 研究沉淀）

> 日期：2026-08-29 ｜ 静态（api13 全包源码）+ 真机（PIE）双重验证
> 用途：编辑器 ↔ 游戏 VM 命令通道的机制底档（bgd_dbg_cmd/bgd_dbg_result 即建立在此之上）

## 命名 VM

运行时存在多个命名 Lua VM（同进程隔离 state）：

| VM | 角色 |
| --- | --- |
| `StateEditor` | 星火编辑器（xdeditor 包，UI 插件、触发器等） |
| `StateGame` | 游戏对局（PIE 调试时游戏 client 即在此 VM；服务端默认在云端调试 host） |
| `StateApplication` | 大厅/登录/支付 UI（startup 包） |

- 判 VM：`lobby.vm_name()`（实测可靠）。**注意**：`__lua_state_name` 全局在 StateEditor/StateGame 实测均为 nil
  （旧知识「C++ 注入 __lua_state_name」与实测不符，已修正——isolation.lua 里 `__lua_state_name == 'StateGame'`
  的判定在 PIE 下因此不命中，内核补丁的 isolation 解锁仍按插槽做）。
- isolation 阉割（io/os/debug 限制）只针对游戏运行时；StateEditor 下完整。

## lobby 原生模块（C 层注册，无 Lua 源）

- 编辑器侧（xdeditor）：`require '@base.base.lobby'`；游戏侧（script 包）：`require '@common.base.lobby'`
  （同一原生模块，两个包路径前缀不同；脚本库内 `common/base/lobby.lua` 又透传 `@base.base.lobby` 并挂 Lua 扩展
  `lobby.app_lua.*`，base_lua_plus 再包一层 base.*）。
- 关键 API（编辑器 VM dump 实测全集节选）：
  - `lobby.vm_name()` → VM 名
  - `lobby.send_luastate_broadcast(name, data)` → 跨 VM 广播（data 支持嵌套表，建议只放原始类型/字符串）
  - `lobby.register_luaState_event(name, fn)` → 接收跨 VM 广播（fn(data)）
  - `lobby.register_event(name, fn)` / `register_once` → VM 内大厅事件
  - `lobby.app_lua` → 跨 VM 可调用函数注册表（Lua 侧挂函数，其他 VM 可触发）
  - 大厅业务类：start_game / return_to_lobby / get_lobby_map_info / quick_start_game / match 系列等
- 注意：个别运行环境（编辑器预览）lobby 原生函数可能缺注册，官方地图代码里有 stub 兜底
  （`if not lobby.send_luastate_broadcast then ... end`，core_mover_td）。**调用一律 pcall**。

## PIE 下双向实测（0.8.0 R0 Q1）

- 编辑器（StateEditor）→ 游戏（StateGame）：`send_luastate_broadcast('bgd_dbg_cmd', {...})` 游戏侧收达 ✓
- 游戏 → 编辑器：游戏侧 `bgd_dbg_result` 广播，编辑器侧 register 收达 ✓
- 链路日志落点：游戏 log `base/base/lobby.lua:353 on_luastate_notify, key[<事件名>], #data_str[<字节>]`。
- 官方实例：StateGame 启动向 StateApplication 广播同步账号（account.lua）；对局设置面板
  `send_luastate_broadcast('back_to_startup')` → 大厅 global_event.lua 接收。

## 落地协议（本项目 dbg 总线）

```
编辑器 → 游戏：bgd_dbg_cmd    { id, cmd, args }      （id 字符串递增）
游戏 → 编辑器：bgd_dbg_result { id, ok, json, result }（result：json=true 时为 JSON 串，否则文本）
```

- 游戏侧端点：bgd 框架 `libs/client/api/dbg_bus.lua`（test_res002 共建验证场，后续随框架下发）。
- 编辑器侧端点：`patches/xdeditor/bgd_mcp_bridge/main.lua`（vm_call：DEFERRED + 3s 无响应超时兜底）。
- 事件名带 `bgd_dbg_` 前缀避免与业务冲突；广播对发送方自身 VM 不可见（实测无需自过滤，但按 id 配对天然免疫）。
