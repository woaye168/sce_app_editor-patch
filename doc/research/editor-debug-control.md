# 星火编辑器「启动/停止调试」控制机制研究

> 研究日期：2026-08-15
> 目的：评估「外部 AI 通过 CLI/MCP 服务控制编辑器启动调试、停止调试」的可行性，并选定通道方案（结论：方案B = C# 注入，详见 csharp-module-injection.md）
> 关联需求：doc/requirements/0.4.0.txt

## 1. 一句话结论

编辑器内「启动调试」「停止调试」本质就是两条已注册的菜单命令，补丁代码直接 `window_title_bar.call_command('调试/调试')` / `call_command('调试/停止')` 即可，等价于用户点菜单，协程/任务管线/错误处理全走官方路径。

## 2. 启动调试（PIE，编辑器内嵌运行）

注册点：xdeditor/ui/menu_bar.lua:1716

```lua
window_title_bar.register('调试/调试', function(item)
    window_title_bar.call_command('调试/停止')   -- 先自动停掉旧局
    ...
    debug_save_as {muti_debug_info = getDebugUserInfo()}
end)
```

内部流程：

1. `debug_save_as{...}`（menu_bar.lua:836+，`co.will_async` 包装）→ 进入 `DebugGame` 任务管线（menu_bar.lua:884-936，`taskPiplineMgr`）。
2. 管线执行 `_debug_save_as(params)`（menu_bar.lua:348-752）：
   - `debug_game_via_remote_host = argv.get('debug_via_remote') ~= '0'`（menu_bar.lua:56，**默认 true**）→ 先 `query_assign_host()` 向 `http://<_G.IP>:9007/api/v1/assign_host` 申请云端调试 host，再 `co.call(DebugManager.update_host, ...)`。
   - `MainFrame:GetDebugMapPath()` 取调试目录 → 清目录 → 按白名单拷贝地图文件（`project_manager.get_project_map_dirs/files`）。
   - `co.call(DebugManager.debug_game, DebugManager, { map_path=..., lua_debug=..., is_trigger_debug=..., game_in_editor=debug_game_in_editor, ... })` 真正拉起（menu_bar.lua:579/601）。
3. `debug_game_in_editor = true` 为默认（menu_bar.lua:49，argv `no_debug_game_in_editor` 关闭）→ 游戏以 **PIE 内嵌窗口**（插件 UI 槽位 `GamePlayInEditor*`）运行。
4. 快捷入口：快捷键 `shortcutMgr.DEBUG_RUN` → `call_command('调试/调试')`（menu_bar.lua:1744）；`'调试/再次调试上次调试版本'`（:1737）复用上次的调试目录跳过生成。

前置条件：地图已打开（`MainFrame:GetMapPath()` 非空），否则走「打开项目」弹窗流程；大厅地图（lobby）不支持此命令。

## 3. 停止调试

注册点：xdeditor/ui/operation_menu.lua:39（在 `EVENT.load_map_done` 后的 init() 里注册，另有一次性兜底 `base.next` 注册，:77-92）

```lua
menu_bar.register_command('调试/停止', function()
    -- 遍历 GamePlayInEditor / GamePlayInEditor1..4 槽位
    sceneMgr:hide_game_in_editor(slot_id, 是否最后一个)  -- 最后一个才销毁服务器游戏局
    pluginMgr:unload_plugin_ui(slot_id)
    pluginMgr:unregister_plugin_ui(slot_id)
    refresh_debug_button_state(false)                    -- C# 侧 SetRunning=false
end)
```

menu_bar.lua:1717 证实 `call_command('调试/停止')` 是官方内部也在用的停止方式（'调试/调试' 的第一步就是它）。

## 4. 状态感知（供服务化时查询用）

- `EVENT.pie_will_launch`：PIE 即将启动事件（operation_menu.lua:94）。
- `pluginMgr:is_plugin_ui_loaded('GamePlayInEditor')`：槽位是否存在即调试局是否在跑（operation_menu.lua:42、:101）。
- `appui.ui.tabs_bar_proxy.register_button_close_event`：调试标签页关闭回调（menu_bar.lua:645/702，operation_menu.lua:98）。
- C# 侧运行状态：`menu_bar.call_cs_function('SetRunning', { value = bool })`（operation_menu.lua:34-36）。

## 5. 无头（headless）调试分支——官方自带「跑一次」CLI

编辑器 exe 带 argv 启动即全自动调试（xdeditor/map_starter/init.lua，由 main.lua:715 登录回调 `require 'map_starter'` 进入）：

1. 加载当前地图 → `trigger_manager.generate_lua_only(map_path)` 生成 lua；
2. 另存到调试目录（`clear_folder` 保留 `.sce_workspace.code-workspace` 与 `.vscode`）；
3. `co.call(DebugManager.debug_game, ...)` 拉起调试（`debug_game_in_editor=false`，:98）；
4. 结束 `os.exit(0)`。

适合 CI 式「跑一次出结果」，**不适合**交互式 AI 反复启停（每次全量冷启动，且无停止通道——进程退出即结束）。

本地调试后门：`_G.__fortest_still_use_local_host = true`（map_starter/init.lua:111）→ host 固定 127.0.0.1:5003，跳过云端 assign_host。

## 6. 通道方案对比（2026-08-15 讨论结论）

| 方案 | 通道 | 结论 |
| --- | --- | --- |
| A 文件队列桥 | 编辑器 Lua 补丁轮询 cmd/resp JSON 文件，外部 MCP 进程转发 | 可行、零风险，但轮询延迟 + 能力上限低（Lua 侧能做的事有限） |
| **B C# 进程内服务（选定）** | 注入自有 C# dll，进程内起 HttpListener（127.0.0.1），直接调 Lua 命令/订阅事件 | **一劳永逸，扩展面最大**；已实测打通（csharp-module-injection.md） |
| C 无头 CLI | 编辑器 exe + generate_and_debug_map argv | 官方自带但只能「跑一次」，无停止通道，弃用 |

方案B 选定理由：编辑器 Lua state 无 TCP server 能力（安装目录无 luasocket dll，`sce.httplib` 仅 HTTP client，`base.debugger` 的 4278 端口属游戏运行时 luadebug 不可用于编辑器）；而 C# 注入后 HttpListener 直接可用，且能订阅编辑器事件做状态推送。

## 7. 服务化落地注意点（0.5.0+ 用）

1. **远端调试默认开启**：`debug_via_remote` argv 默认非 '0' → 启动调试会先走云端 assign_host（网络依赖、可能排队失败）。AI 自动化场景应评估固定本地 host 或确认云端链路稳定性。
2. **错误弹窗会卡流程**：`_debug_save_as` 内 `error(...)` 会弹 message_window 等人点。服务化时必须包 xpcall 并考虑把错误文本经事件/日志回传，必要时 hook 弹窗。
3. **call_command 的获取方式**：`window_title_bar` 在 menu_bar.lua 顶部 require；补丁侧取法需在实现时验证（`require 'ui.window_title_bar'` 或经 menu_bar 转发），也可由 C# 侧经 CSharpLua 互操作调 Lua 函数。
4. **必须先开图**：'调试/停止' 在 load_map_done 后才注册；服务应暴露「地图是否就绪」状态位。

## 8. 追加核实（2026-08-16，0.4.0 范围扩编时验证）

### 8.1 官方已注册命令全清单（服务方法池）

`window_title_bar.register(...)` 在 menu_bar.lua 注册了 **80+ 命令**，`call_command` 可触发（register 与 register_command 殊途同归进 callback_map，实证：menu_bar.lua:1745 快捷键 `call_command('调试/调试')` 触发的是 register 注册的项）。关键可用项：

- 文件：'文件/打开'(:1203)、'文件/新建'(:1268)、'文件/保存'(:1299)、'文件/保存数编'(:1405)、'文件/强制重新加载项目'(:1378)、'文件/项目管理器'(:1614)
- 调试：'调试/调试'(:1716)、'调试/再次调试上次调试版本'(:1737)、'调试/调试(本地服务器)'(:2124)、'调试/调试(启动LuaDebug)'(:2211)、'调试/重新生成并调试'(:2321)、'调试/模拟多人调试'(:1806)、'调试/多开调试'(:2467) 等数十个变体
- 发布/工具：'发布/发布项目'(:2607)、'工具/插件'(:2631)、'工具/启动代码编辑器'(:2717) 等

推论：服务方法不需要逐个实现，做一个 **call_command 通用透传** + **list_commands（wrap register/register_command 记录命令表）** 即全覆盖。

### 8.2 错误弹窗抑制点（已核实）

`ui/components/message_window.lua`：模块级单点 `message_window(func, btn_text, prompt_text, title_text, ui_size, slot_id)`（:138），`current_window` 单例（:21），导出 OptEnum（:4，Confirm/Close 等）。官方所有弹窗都经此函数。抑制方案：Lua 补丁 `require 'ui.components.message_window'` 后 wrap 该函数——受控开关开启时不弹窗、以默认 opt 直接回调 func、把 {title, prompt} 经事件桥推送外部。

### 8.3 C# 可订阅的原生事件（Event 枚举，scemodule.dll SCEModule.Interface/Event.cs）

`Update` / `E_LoadMap`（地图加载）/ `CreateModule` / `ActivateWindow` / `ExitEditor` / `UpdateProgress` / `ShowMessageBox`（**弹窗感知兜底**）/ `NotifyCSharpTriggerEditor` / `AnimationEditorShow` / `CreateNewProfiler` / `MaterialEditorShow`。订阅入口 `ApplicationExport.SubscribeToEvent(Event, Delegate)`（ApplicationExport.cs:67，内部按 EnumValueExtension 转字符串走 EventObject）。
