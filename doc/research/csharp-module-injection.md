# 星火编辑器 C# 扩展模块注入研究（已实测打通）

> 研究日期：2026-08-15 ~ 2026-08-16
> 状态：**冒烟测试通过**——自定义 C# DLL 成功在编辑器进程内加载并执行（WinUI 窗口弹出 + 探针日志 + PID 实证）
> 关联需求：doc/requirements/0.4.0.txt

## 0. 一句话结论

星火编辑器内嵌 CoreCLR（.NET 9 自包含），其主程序集 sce.dll 里有一个**泛型反射事件入口**（`Event.ActivateWindow` → `Type.GetType` + `Activator.CreateInstance`），Lua 侧可用 `SCE.Common.csharp_activate_window('命名空间.类名, 程序集名')` 触达。只要把我们的 dll 放进引擎版本目录并在 `sce.deps.json` 登记，就能让任意自有 C# 代码在编辑器进程内运行，**不需要修改任何官方二进制文件**。

## 1. 运行环境勘察（version-13 目录）

编辑器运行目录（如 `D:/sce_online`）按 api 版本分目录存放引擎：`version-12` / `version-13` / `version-2000`（api 13 → version-13）。

version-13 关键构成：

| 文件 | 说明 |
| --- | --- |
| coreclr.dll / clrjit.dll / clrgc.dll | CoreCLR 运行时（.NET 9.0.13，自包含） |
| hostfxr.dll / hostpolicy.dll | .NET 宿主（native → 托管引导） |
| sce.runtimeconfig.json | tfm=net9.0，includedFrameworks=Microsoft.NETCore.App 9.0.13 |
| sce.deps.json | 程序集解析清单（默认 AssemblyLoadContext 的唯一解析依据） |
| sce.dll | 主编排程序集（3.2MB，含模块分派/App 入口） |
| scemodule.dll | 官方 C# 模块集（PlayerSettings/TitleMenuBar 等） |
| scecustomcontrol.dll / winuiedit.dll | 控件库 / 编辑控件 |
| microsoft.winui.dll / microsoft.ui.xaml.dll | WinUI 3（文件版本 3.1.6 ≈ Windows App SDK 1.6.x，deps 记录 1.6.250205002） |
| system.net.httplistener.dll / system.net.sockets.dll | **HTTP/TCP 服务能力天然具备** |
| lua54.dll / sceengine.dll（49MB）/ themis_x64.dll | native 引擎 |

## 2. 官方模块机制（Lua → C#）

xdeditor 库中收集到的全部 `SCE.Common.create_csharp_module(...)` 调用（23 处，约 20 个模块名）：

- 高频：`'EditorMainWindow'`（menu_bar.lua:77 启动即建）、`'ObjEditor'`、`'ProfilerClient'`、`'PlayerSettings'`、`'EditorSettings'`、`'ResourceLibrary'`、`'SceAssistant'`、`'MutiDebugWindow'`、`'AnimationEditor'`、`'MaterialEditor'`、`'SceneStatistics'`、`'BuildSettings'`、`'BloodStripEditor'`、`'TriggerEditor'`、`'EditorApiSetting'`、`'ProjectApiSetting'` 等。

调用链：

```
Lua: SCE.Common.create_csharp_module('Name')
  → sceengine.dll（native；内含 create_csharp_module 实现与 8 个硬编码胶水 CSharp_*.cpp）
  → 触发托管事件 Event.CreateModule（ModuleType=名字）
  → sce.dll App.OnLaunched 注册的 switch 分派（反编译 SCE/App.cs:181-213）
      "EditorMainWindow" => Editor.GetOrCreateModule<EditorMainWindow>(), ...
      _ => null    ← 未知名字直接丢弃，不可扩展
```

官方模块类约定（反编译 scemodule.dll 实证）：

```csharp
public sealed class PlayerSettings : WindowSCE, IServiceModule, IComponentConnector { ... }
```

即 WinUI 3 窗口 + `IServiceModule` 标记接口 + DI 注册（`AddToService(IServiceCollection)`）。

## 3. 关键注入点（核心发现）

sce.dll `App.cs:214-217` 还订阅了另一个事件 **`Event.ActivateWindow`**，处理逻辑是泛型反射而非白名单：

```csharp
ApplicationExport.SubscribeToEvent(Event.ActivateWindow, delegate(EventData eventData)
{
    ((Window)System.Activator.CreateInstance(
        Type.GetType(eventData.GetValue("ModuleType").GetString())))?.Activate();
});
```

Lua 侧对应 API 有官方注释残留实证（xdeditor/ui/menu_bar.lua:2879）：

```lua
--     SCE.Common.csharp_activate_window('SCE.Module.YAWinUIDemo.YAWinUIDemo')
```

实测结论：

- **必须传程序集限定名**（`'BgdBridge.BridgeWindow, BgdBridge'`）；不带程序集名时 `Type.GetType` 只在 sce.dll（调用程序集）内查找。
- 入口类继承 `Microsoft.UI.Xaml.Window` + 无参构造函数即可被创建并被 `Activate()`（弹窗实测成功）。
- `Type.GetType` 失败返回 null → `Activator.CreateInstance(null)` 抛 `ArgumentNullException`：实测**不会崩编辑器**（异常被官方事件系统吸收），但 Lua 侧 `ok=true` 不代表建成，验证必须看 C# 侧探针。

## 4. 程序集解析（我们的 dll 如何被找到）

CoreCLR 默认 AssemblyLoadContext 只按 `sce.deps.json` 解析（无额外探测路径）。已实测可行的登记方式：

1. `bgd_mcp_bridge.dll` 复制进 `version-<api>/`（与 sce.dll 同目录）。
2. `sce.deps.json` 追加两处（备份为 `sce.deps.json_bak` 后再改，幂等）：
   - `targets[".NETCoreApp,Version=v9.0/win-x64"]["bgd_mcp_bridge/1.0.0"]`：
     ```json
     { "runtime": { "bgd_mcp_bridge.dll": { "assemblyVersion": "1.0.0.0", "fileVersion": "1.0.0.0" } } }
     ```
     注意所有条目都在 **win-x64 子组**（顶层 `.NETCoreApp,Version=v9.0` 组为空）。
   - `libraries["bgd_mcp_bridge/1.0.0"]`：`{ "type": "project", "serviceable": false, "sha512": "" }`
3. 无需声明 dependencies——我们引用的 `Microsoft.WinUI` / `WinRT.Runtime` / `Microsoft.Windows.SDK.NET` 已在宿主依赖图中。

## 5. C# 侧编译方式（实测踩坑）

- **不要用 WASDK NuGet 包**：`Microsoft.WindowsAppSDK 1.6` 的 `MrtCore.PriGen.targets` 在 dotnet SDK 10.0.301 上引用不存在的 `Microsoft.Build.Packaging.Pri.Tasks.dll`（ExpandPriContent 任务），构建直接失败。
- **实测可行**：csproj 直接 `<Reference>` 宿主 version-13 自带的投影程序集（`microsoft.winui.dll` / `winrt.runtime.dll` / `microsoft.windows.sdk.net.dll`），`TargetFramework=net9.0-windows10.0.19041.0`，零 NuGet 依赖，编译 3 秒通过，且版本与宿主严格一致。
- 项目骨架：仓库 `csharp/BgdBridge/`（0.4.0 正式版改名为 bgd_mcp_bridge）。

## 6. Lua 侧调用要点（实测踩坑）

- **SCE 不是全局**：xdeditor 各文件均为 `local SCE = ImportSCEContext()`（main.lua:83 也是 local）。补丁模块必须自行获取，否则报 `attempt to index a nil value (global 'SCE')`。
- **`base.timer` 签名是 `(timeout, count, on_timer)`**：两参调用会把函数塞进 count，触发 `common/base/timer.lua:107: attempt to compare number with function`（弹窗「@cxt, on_timer函数为空」）。单次延迟用 **`base.wait(timeout, fn)`**。
  - 附带发现：xdeditor 官方 main.lua:62 的 `base.timer(2*60*1000, function()...)` 同款两参调用疑似官方 bug（generate_cmd 分支，正常启动不触发）。
- **时序**：补丁入口在编辑器启动流程中执行，托管侧事件订阅（`App.OnLaunched`）未必就绪。实测 `base.wait(5000)` 后一次调用成功；正式模块保留「延迟 + pcall 失败重试（3s×5）」保护。

## 7. 冒烟测试实录（2026-08-16）

部署物：

| 位置 | 内容 |
| --- | --- |
| `version-13/BgdBridge.dll` | 冒烟测试 dll（BridgeWindow : Window，ctor 弹窗 + 写探针日志） |
| `version-13/sce.deps.json` | 注入 BgdBridge/1.0.0 target + library 条目 |
| `version-13/sce.deps.json_bak` | 原始备份 |
| xdeditor 补丁目录 `sce_app_editor-patch/bgd_bridge_test/main.lua` | Lua 触发模块（base.wait 5s → csharp_activate_window，pcall 重试） |
| xdeditor 补丁入口 `sce_app_editor-patch/main.lua` | modules 表追加 'bgd_bridge_test' |

结果：编辑器启动约 5 秒后弹出「BgdBridge」窗口；探针日志：

```
[BgdBridge] BridgeWindow created at 2026-08-16T00:28:18.9265857+08:00, pid=31100
```

排障过程（供后续参考）：base.timer 签名错误 → 崩 on_tick；SCE 全局 nil → pcall 重试日志记录 attempt 1-5 全失败，改 `local SCE = ImportSCEContext()` 后成功。

## 8. 反编译证据与工具

- 工具：`dotnet tool install -g ilspycmd`（1.0.0.9375）。
- 反编译产物（本机临时）：`D:/sce_online/Res/maps/bgd_glzy/.tmp_verify/decomp/{sce,scemodule}/`。
- 关键文件：`SCE/App.cs`（Event.CreateModule switch :181-213、Event.ActivateWindow 泛型反射 :214-217、DI 注册表 :100-142）；`SCEModule/Editor.cs`（GetOrCreateModule :55、`Editor.GetService<T>` DI 入口）。
- native 侧：`create_csharp_module`、`csharp_activate_window`、`CSharp_*.cpp`（8 个硬编码胶水）字符串均在 sceengine.dll。

## 9. C# ↔ Lua 双向事件桥（2026-08-16 追加，已核实）

服务化（HTTP/MCP）必须让 C# 与 Lua 互调。官方事件总线支持**任意字符串事件名**，双向均有实证：

| 方向 | 发送方 | 接收方 | 官方实证 |
| --- | --- | --- | --- |
| C# → Lua | `Editor.GetService<EventManager>().SendEvent('事件名', jsonStr)`（EventManager.cs:5 → native EventManager_SendEvent） | Lua `SCE.GetEventManager():register_event('事件名', fn)` | `CS_muti_debug`（MutiDebugWindow.cs:701 → menu_bar.lua:1752） |
| Lua → C# | Lua `GetMainFrame():SendEvent('事件名', {table})` | C# `Editor.GetService<EventObject>().SubscribeToEvent('事件名', handler)`（Object.cs:56，字符串事件名） | `EditorMainTitleMenuBar`（menu_bar.lua:1041 → EditorMainWindow.cs:142） |

推论：bgd_mcp_bridge 的 Lua 模块与 C# dll 之间可以建请求-响应闭环（C# SendEvent 'bgd_mcp_cmd' → Lua 执行菜单命令 → Lua SendEvent 'bgd_mcp_ack' 带 id 回传 → C# 关联响应）。

## 9.5 隐藏窗口方案（2026-08-16 追加，编译已验证/实测待重启）

宿主 ActivateWindow 处理器会无条件 `Activate()` 我们的窗口。自隐藏方案：ctor 订阅 `Activated += (_, _) => AppWindow.Hide()`。
- 编译注意：`AppWindow`（Microsoft.UI.Windowing）需额外引用宿主 `microsoft.interactiveexperiences.projection.dll`（仅 microsoft.winui.dll 不够，否则 CS0012）。
- 冒烟 dll 已按此改造并重新部署，实测预期：窗口不残留可见、探针日志含 'hidden-mode test'。

## 10. 能力边界评估

**能做到**：编辑器内任意 WinUI 3 UI、进程内任意服务（HTTP/MCP/文件监听/子进程）、调用 `SCE.CppInterface.*` 全部 native 导出封装（EventManager/DataEditor/Score/Goods…）、`Editor.GetService<T>` 取托管侧全部 DI 服务、经事件总线与 Lua 全双工互操作（见 §9；注意 `CSharpLua` 类是资源库专用封装，并非通用 Lua 互操作入口）。

**折扣**：① native 能力边界 = 官方已导出边界，未导出函数需 detour（不建议，触碰安全红线）；② UI 操作须在 WinUI 线程；③ 生命周期随编辑器升级失效（重新应用补丁即可）；④ 入口依赖 sce.dll 的 ActivateWindow 泛型处理器，官方若移除需另找入口（短期稳，属官方自留调试口）。
