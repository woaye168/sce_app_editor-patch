# debug-muti-debug-crash —— 模拟多人调试崩溃定位

## 症状
- 应用补丁后：编辑器「调试/模拟多人调试」报错退出（MutiDebugWindow ctor → AddMapScoped<IDataCore> → CurrentMapName is null）
- 取消补丁后：正常
- **注释掉 bgd_mcp_bridge/main.lua 全部代码后：正常**（用户实测）

## 已排除
- 不是官方原生 bug（取消补丁不崩）
- 不是过早 require（0.4.3 已把 require 推迟到 load_map_done，仍崩）

## 假设（可证伪）
- H1：`EDITOR.event_register(EVENT.load_map_done, init_deferred)` 的回调在官方 load_map_done 处理链中间执行 require，打断 CurrentMapName/DataEditor 状态设置。【观测：把 init_deferred 移出 load_map_done 回调后是否还崩】
- H2：`register_event('bgd_mcp_cmd')` 污染了 native 事件表。【观测：去掉该注册后是否还崩】
- H3：C# dll 激活后 `Editor.GetService` 触碰 DI 污染 AddMapScoped 缓存。【观测：ACTIVATE_CSHARP=false 隔离版是否还崩】
- H4：message_window/menu_bar 的 wrap 改变了官方行为。【观测：注释掉两个 wrap 后是否还崩】

## 根因确认（2026-08-16，用户二分定位）
用户实测：**只要注释 main.lua 中激活 C# 的那一行（csharp_activate_window 调用），多人调试就不再崩**。
→ 证实 H3：破坏者是 **C# dll 激活后，BridgeWindow 的 InitServicesAsync 在地图未加载时轮询
`Editor.GetService<EventManager/EventObject>()`**，该 DI 解析污染了 `AddMapScoped` 的共享缓存
`mapScopedServices`（Extension.cs:20），导致官方 MutiDebugWindow 后续解析 `IDataCore`
（AddMapScoped 无占位，CurrentMapName==null 时抛 InvalidOperationException）崩溃退出编辑器。

排除 H1/H2/H4（require 时机、register_event、wrap 均非元凶——0.4.3 延迟 require 后仍崩）。

## 修复
C# BridgeWindow.InitServicesAsync：DI 触碰（GetService）前增加 `WaitForMapLoadedAsync()`——
轮询等待 `Editor.CurrentMapName` 非空（读静态属性不触发 DI 解析），地图就绪后才 GetService。
无图时服务不启动（本来无图也无法 start_debug，符合预期）。

## 状态
[待用户验证] 修复版已构建，待确认「模拟多人调试」不再崩。
