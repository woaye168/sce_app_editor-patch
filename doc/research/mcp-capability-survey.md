# 0.5.0 前置研究：SCE.CppInterface 与 DI 服务能力面勘察

> 研究日期：2026-08-16
> 关联方案：doc/requirements/0.5.0.txt（海量工具 MCP 架构）
> 素材：ilspycmd 反编译 version-13 的 sce.dll / scemodule.dll（`D:/sce_online/Res/maps/bgd_glzy/.tmp_verify/decomp/{sce,scemodule}/`）
> 结论先行：**原方案「通用反射调用一切」需修正——CppInterface 大多不是 JSON 可直接调的形态；真正的主战场是 DI 单例服务（svc.*）+ 少量手写高层封装（数编）**

## 1. CppInterface 规模与两种形态

sce.dll 的 `SCE.CppInterface` 命名空间：**688 个 .cs，其中 273 个 `*Export` 类**。全部是 SCEEngine.dll 的 P/Invoke 封装（`[LibraryImport("SCEEngine.dll")]`），分两种形态：

### 形态 A：实例包装类（绝大多数）

```csharp
public class CSharpScore : DoxygenObject {   // Object 体系，包 nativePtr_
    public CSharpScore(Context context) { nativePtr_ = CSharpScoreExport.Create(context.GetPtr()); }
    public void InitScore(string mapName, int uid, HandleScoreInit cb) { ... }
}
```

- 方法体 = 把 `nativePtr_` 传给对应 `XxxExport` 静态方法。
- **实例无法从 JSON 凭空构造**（要 native 指针）；只能靠 DI 容器里已有的单例，或 `Context` 创建（Context 本身也包 nint，来源是宿主初始化）。

### 形态 B：纯静态 P/Invoke 类（少数）

典型 `SCE.CppInterface.DataEditor.DataEditor`（静态类，~60 个 `DataEditor_*` 静态方法）。但签名大量是 **nint 指针流**：

```csharp
DataEditor_GetEntryNode_Lua(string fullLink) → nint      // 拿节点指针
DataEditor_ObjGetPairCount(nint ptr) → nint
DataEditor_PairGetValue_Val(nint ptr) → object?
DataEditor_InitEntryNodeSetValueArg_String(string link, nint pair, mode, string val) → nint
DataEditor_DoEntryNodeSetValue_Lua() → bool
```

- 读写一个数编值 = 「取节点 → 遍历 pair → 转 val」多步有状态指针操作。
- **通用单方法反射调用对 nint 参数没有意义**（外部没有合法指针可传）。

## 2. 数编读写的正确路线：IDataCore（托管高层 API）

scemodule.dll 的 `SCEModule.DataCore.Interface.IDataCore` 才是给人用的数编 API（PlayerSettings 等官方模块全走它，`Editor.GetService<IDataCore>()`）：

```csharp
public interface IDataCore {
    IEntryNode DefaultGameplayEntry { get; }
    IEntryNode DefaultMapConfigEntry { get; }
    IEntryNode GetEntryNode(IDataLink link);
    bool AddGameChange(string link, List<object?> path, IDataType data);
    bool CommitChanges();
    IDataType MakeBool/MakeInt/MakeDecimal/MakeString/MakeText/MakeNull/MakeList/MakeProp(...);
}
```

- DI 注册：`services.AddMapScoped<IDataCore, DataCore>()`（sce.dll App.cs:111）——**地图加载后才有实例**（map-scoped）。
- 结论：数编能力**不做通用反射**，而是手写一小批高层封装能力（如 `datacore.read(link)` / `datacore.write(link,path,value)` / `datacore.list_entries()`），内部走 IDataCore；这比暴露 60 个 nint 方法对 AI 友好几个数量级。

## 3. DI 服务清单（svc.* 的真实水面）

注册入口两处：sce.dll `App.ConfigureServices`（引擎级，App.cs:86-117+）与 scemodule `Editor.xaml.cs ConfigureServices`（模块窗口级，GetService 报错信息实证）。

引擎级单例（App.cs 实证）：

| 服务 | 价值评估 |
| --- | --- |
| `FileSystem` | **极高**：FileExists/ScanDir/Copy/ReadDir/GetResourceDir… 几乎全 string/bool/int 签名，JSON 直接可调 |
| `EditorSettingsManager` | 高：编辑器设置读写 |
| `SCE.CppInterface.EventManager` | 已在用（SendEvent） |
| `SceneManager` / `PluginsManager` / `VirtualWindowManager` / `FWindowManager` | 中：状态查询类 |
| `ApiManager` / `ApplicationArgs` / `Time` / `Input` | 中低 |
| `CSharpScore` / `CSharpScoreDataManager` / `CSharpGoods` | 低：游戏局运行时对象，编辑器态大概率无意义 |
| `CSharpLua` | **资源库专用封装**（csharp-module-injection.md §10 已注明），非通用 Lua 入口 |
| `IDataCore`（map-scoped） | **极高**：数编读写，见 §2 |

调用方式（反射）：`SCEModule.Editor.Current`（静态属性）→ `.Services.GetService(Type)` → `MethodInfo.Invoke`。泛型 `GetService<T>` 不可用反射直接调，但 `IServiceProvider.GetService(Type)` 等价。

抽查 `FileSystem` 全部公开方法：~40 个方法中绝大多数参数为 string/bool/int/uint/List\<string\>，**JSON 可表达率估计 >85%**——svc.* 通用反射调用的可行性实证成立。

## 4. 对方案的修正（已并入 0.5.0.txt 终稿）

| 原方案 | 修正后 |
| --- | --- |
| CppExecutor 通用反射调用 SCE.CppInterface.* | 降级为**次要通道**：只收「静态方法 + 全基元参数」的条目（如 `DataEditor_IsMapLoaded_Lua(bool)` 这类）。实例类（nativePtr_ 体系）不进目录——外部构造不了实例 |
| svc.* 反射调用 | 升为**主力通道**：DI 单例 + 基元签名 = 天然 JSON 可调，FileSystem/EditorSettings/IDataCore 高价值能力都在这里 |
| 数编 = CppInterface 方法暴露 | 改为**手写高层封装** `datacore.*`（走 IDataCore，map-scoped，无图时返回「地图未打开」） |
| 目录规模「数千条」 | 修正为「数百条」：svc 单例方法 + cpp 静态基元方法 + cmd 命令 + lua 方法 + 手写封装 |

## 5. 实施注意点（新发现）

1. **AddMapScoped 污染红线**：BridgeWindow 现有设计已把 DI 触碰推迟到地图加载后（WaitForMapLoadedAsync），svc.* 执行器必须复用同一纪律——map-scoped 服务在无图时 GetService 会触发官方 bug 路径（MutiDebugWindow 崩溃前科，见 patches/xdeditor/bgd_mcp_bridge/main.lua 注释）。
2. **实例方法返回值**：FileSystem.ScanDir 返回 `List<string>` 等，序列化没问题；返回引擎对象（IEntryNode 等）的做浅投影/ToString。
3. **nint 返回值的方法**（即使是静态）一律标 `unsupported`——指针透出给 AI 无意义且危险。
4. DI 服务完整清单可在运行时枚举？**不能**（IServiceProvider 无枚举 API）——清单必须从反编译静态维护进 catalog（App.cs + Editor.xaml.cs 两处注册表），编辑器升级后重扫。

## 6. 架构评审补充知识点（2026-08-16 v3 评审落盘）

### 6.1 svc 通道的实证盲区：只验证了参数侧，没验证返回侧/副作用侧

本研究 §3 的「JSON 可表达率 >85%」只覆盖**方法签名（入参）**。两个未实证的盲区：

1. **返回侧**：DI 单例相当多方法返回引擎对象（IEntryNode、窗口/场景句柄类）。浅投影时遍历其属性 getter，**某些 getter 会触 native 调用，错误时序下可能直接崩编辑器进程**——这是 native AV，不是托管异常，「跳过求值抛异常的属性」的兜底防不住它。
2. **副作用侧**：服务单例是全局共享状态，非查询方法的真实副作用面无法从签名判断（PopQueue/GetOrCreate 式命名陷阱）。

**结论（已并入 0.5.0 v3）**：svc 目录实行**服务级准入制**——默认整体不开放，annotations.json 逐服务人工准入。准入动作 = 在隔离地图里对该服务每个 read 级方法逐个实调确认无崩溃。对 svc 的安全模型是「未知即不开放」，比一般条目的「未知即 write」更严。

### 6.2 IDataCore.CommitChanges 可撤销性：**已实证——不可撤销**（2026-08-17 真机）

实证方法：隔离地图（test_res002）经 `datacore.write`（auto_commit=true）提交 `opened_slots=[9]` → `SCE.GetUndoRedoManager()` `can_undo()=true` → 连续 `undo()` 至栈空（3 次）→ 回读仍为 `[9]`。**CommitChanges 落盘的修改不进 UndoRedoManager 撤销栈，Ctrl+Z 不可撤销。**

已据此定稿（0.5.0 自测自修轮落地）：

- `Executors.cs` WriteAsync/BatchWriteAsync 的 auto_commit 默认 **false**（AI 必须显式 `auto_commit=true`；暂存未提交重开地图即丢弃，构成天然回滚）
- annotations.json `datacore.write` / `datacore.batch_write` risk 升 **danger**（需 config.json `danger_allow` 放行）

附带实证发现（同轮修复）：

- `DataDecimal.ChangeValue` 把 decimal 直传 P/Invoke，而 `DataValMarshaller` 封送白名单（bool/short/int/long/float/double/string）**不含 decimal**，必抛 `InvalidDataException`——引擎侧 Bug。小数写入改走自实现 `DataDouble`（IDataType 是公开接口，double 在白名单内）。
- `DataCore.AddGameChange` 内部固定定位 `IsMatchKey("Game")` 的顶层 pair，**写路径相对 Game 对**（官方调用方 PlayerSettings 传 `["player_setting"]` 不带 Game 前缀）；执行器兼容剥掉首段 "Game"。
- 文本（多语言）字段需 `MakeText`（ENSVM_PASTE + Default 子键），`MakeString` 直写返回 false；执行器对字符串值自动 fallback MakeText。
- 数编**不允许新建 schema 外字段**，只能写既有字段。
- **编辑器对 first-chance 异常记 `logs/CSharp/Exception/` 日志并弹「发生异常」模态**（即使异常被 catch），模态期间 BridgeWindow DispatcherQueue 不泵、全部 UI 调用超时——桥内业务错误一律返回错误节点，绝不抛异常。
- SCE 数编 list 为 **1 基**（`{"1":9}`），读投影按 1 基/0 基连续 int 键识别数组。

### 6.3 版本漂移的检测只是半个闭环

catalog 静态生成 + 运行期惰性校验，只能让 AI 收到「当前编辑器版本不可用」。但**人没有任何渠道感知漂移已发生**。v3 已补：漂移时置持久 warning 状态，推 `/events`（`catalog_version_mismatch`）+ search 结果头部固定标注。实现注意：catalog 头记录的引擎版本号来源要与 api_pak_version.json / sce.dll 程序集版本对齐，启动时一次性比对成本最低。

### 6.4 现有实现基线事实（评审时核实，供开发对照）

- 现网 tools/list 为硬编码 10 个固定工具 + cmd_uXXXX slug 动态工具（McpServer.cs BuildToolsList/ToToolSlug），v3 全部删除重写为 Gateway 分发。
- UI 线程调度现状：TryEnqueue 仅 fire-and-forget（EditorBridge.SendOnUiThread/SendMenuCommand），**「UI 线程执行并取回返回值」的 TryEnqueue+TCS 组合尚不存在**，是 M3 的新增核心机制；现有 TCS 模式（_pending ack 配对，RunContinuationsAsynchronously）可复用其模式但不能复用实例。
- JSON 序列化全用 System.Text.Json 默认选项（中文必转义），无统一出口 helper——M1 要先建统一出口再改各调用点。
- Lua 桥 ack 的 data 是 json.encode 字符串（VariantMap 会把 Lua 表变 StringVector 的规避约定，main.lua:215-224 注释），C# 侧已 Parse 成对象，M1 只需保证最终出口不再二次转义。
- 全仓库无审计日志机制（grep audit 零命中），audit 是全新组件。
- 同步阻塞仅 1 处：GetCommandToolMap 的 GetAwaiter().GetResult()（HTTP 线程池线程上，无 UI 死锁风险但阻塞），slug 机制删除时随之消失。
