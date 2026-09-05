// make_catalog：能力目录生成工具（0.5.0 M5，examples/ 同款定位的构建期工具）。
// 反射扫描 sce.dll / scemodule.dll，产出骨架 catalog.json：
//   svc.* —— DI 单例服务公开实例方法（服务清单静态维护自两处 DI 注册表：
//            sce.dll App.cs ConfigureServices + scemodule Editor.xaml.cs；
//            IServiceProvider 无运行时枚举 API，工厂/扩展注册的盲区靠运行期惰性校验兜住）
//   cpp.* —— SCE.CppInterface 静态方法中「全基元/枚举参数 + 非 nint 返回」的条目
//   datacore.* / lua.* / sys.* —— 手写静态条目（本文件 StaticEntries）
// 用法（0.5.3 起不再内置本机路径默认值，两种用法必传其一）：
//   dotnet run --project csharp/make_catalog -- --project <项目根> [<输出路径>]
//     从项目经 locate 链推导宿主目录（map_settings.json api_version + tsconfig.json
//     typeRoots → 编辑器根 → 上两级运行根 → version-<api>）
//   dotnet run --project csharp/make_catalog -- <宿主目录> <api版本> [<输出路径>]
//     显式指定，如 D:/sce_online/version-13 13
//   输出默认：../bgd_mcp_bridge/catalog.json
// 编辑器升级后重跑本工具（make_slots 同款「版本更新后重跑」约定）。

using System.Reflection;
using System.Runtime.Loader;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;

static string? DeriveHostDirFromProject(string projectRoot, out string? apiVersion)
{
    apiVersion = null;
    // 1. map_settings.json → api_version（对象或数字两种形态）
    var msPath = Path.Combine(projectRoot, "project", "map_settings.json");
    if (!File.Exists(msPath)) return null;
    var ms = JsonNode.Parse(File.ReadAllText(msPath)) as JsonObject;
    var av = ms?["api_version"];
    apiVersion = av is JsonObject o ? o["api_version"]?.ToString() : av?.ToString();
    if (string.IsNullOrEmpty(apiVersion)) return null;

    // 2. script/tsconfig.json → typeRoots 任意一条含 /Res/_m/ 的路径，前缀即编辑器根
    var tsPath = Path.Combine(projectRoot, "script", "tsconfig.json");
    if (!File.Exists(tsPath)) return null;
    var tsText = File.ReadAllText(tsPath);
    // typeRoots 值是 JSON 字符串，直接正则取含 /Res/_m/ 的字符串值前缀（兼容正反斜杠由调用方归一化）
    var m = System.Text.RegularExpressions.Regex.Match(tsText.Replace('\\', '/'), @"""([^""]*?)/Res/_m/", System.Text.RegularExpressions.RegexOptions.IgnoreCase);
    if (!m.Success) return null;
    var editorRoot = m.Groups[1].Value.Replace('\\', '/').TrimEnd('/');
    if (string.IsNullOrEmpty(editorRoot)) return null;

    // 3. 编辑器根上两级 = 运行根，hostDir = <运行根>/version-<api>
    var engineRoot = Path.GetDirectoryName(Path.GetDirectoryName(editorRoot));
    if (engineRoot == null) return null;
    return Path.Combine(engineRoot, $"version-{apiVersion}");
}

string hostDir;
string apiVersion;
string outPath;
if (args.Length >= 1 && args[0] == "--project")
{
    if (args.Length < 2)
    {
        Console.Error.WriteLine("用法: make_catalog --project <项目根> [<输出路径>]");
        return 1;
    }
    var derived = DeriveHostDirFromProject(args[1], out var av);
    if (derived == null || av == null)
    {
        Console.Error.WriteLine($"无法从项目推导宿主目录（检查 project/map_settings.json 与 script/tsconfig.json）: {args[1]}");
        return 1;
    }
    hostDir = derived;
    apiVersion = av;
    outPath = args.Length > 2 ? args[2] : DefaultOutPath();
    Console.WriteLine($"已从项目推导：hostDir={hostDir} api={apiVersion}");
}
else if (args.Length >= 2)
{
    hostDir = args[0];
    apiVersion = args[1];
    outPath = args.Length > 2 ? args[2] : DefaultOutPath();
}
else
{
    Console.Error.WriteLine("用法: make_catalog --project <项目根> [<输出路径>]  |  make_catalog <宿主目录> <api版本> [<输出路径>]");
    return 1;
}

// 默认输出 <repo>/csharp/bgd_mcp_bridge/catalog.json：从 BaseDirectory 向上找 make_catalog 目录定位
//（固定上溯层级不可靠——x64 平台下 bin/x64/Debug/net9.0 多一级，曾把 catalog 生成到仓库根）
static string DefaultOutPath()
{
    var dir = new DirectoryInfo(AppContext.BaseDirectory);
    while (dir != null && dir.Name != "make_catalog") dir = dir.Parent;
    var csharpDir = dir?.Parent;
    return Path.GetFullPath(Path.Combine(csharpDir?.FullName ?? ".", "bgd_mcp_bridge", "catalog.json"));
}

if (!Directory.Exists(hostDir))
{
    Console.Error.WriteLine($"宿主目录不存在: {hostDir}");
    return 1;
}

// 独立 ALC + 宿主目录解析：只取元数据（GetTypes/GetMethods），不执行宿主代码
var alc = new AssemblyLoadContext("inspect");
alc.Resolving += (ctx, name) =>
{
    if (name.Name == null) return null;
    var p = Path.Combine(hostDir, name.Name + ".dll");
    if (File.Exists(p))
    {
        try { return ctx.LoadFromAssemblyPath(p); } catch { }
    }
    return null;
};

Assembly Load(string name) => alc.LoadFromAssemblyPath(Path.Combine(hostDir, name));

var sce = Load("sce.dll");
var scemodule = Load("scemodule.dll");

var capabilities = new List<JsonObject>();

// ---------------- svc.*：DI 单例服务清单（静态维护自 sce.dll App.cs ConfigureServices） ----------------
// 注意：只收引擎级单例；AddMapScoped（IDataCore）走 datacore.* 手写封装；瞬态窗口/VM 无外部调用价值。
(string Type, string Assembly)[] svcServices =
[
    // 0.8.0 起从 svc 清单剔除 SCE.CppInterface.FileSystem：文件读删/复制/改时间等 AI 用自身
    // 工具即可完成，不需要 MCP 通道（减少搜索噪音）。注：下方 cpp.* 静态方法全量扫描仍会发出
    // 4 条 cpp.FileSystem.GetXxxDir 只读目录 getter——无写能力、噪音可忽略，有意保留。
    ("SCE.CppInterface.EditorSettingsManager", "sce"),
    ("SCE.CppInterface.SceneManager", "sce"),
    ("SCE.CppInterface.PluginsManager", "sce"),
    ("SCE.CppInterface.VirtualWindowManager", "sce"),
    ("SCE.CppInterface.FWindowManager", "sce"),
    ("SCE.CppInterface.ApiManager", "sce"),
    ("SCE.CppInterface.Input", "sce"),
    ("SCE.CppInterface.Time", "sce"),
    ("SCE.CppInterface.EventManager", "sce"),
    ("SCE.CppInterface.CSharpLua", "sce"),
    ("SCE.CppInterface.CSharpScore", "sce"),
    ("SCE.CppInterface.CSharpScoreDataManager", "sce"),
    ("SCE.CppInterface.CSharpGoods", "sce"),
    ("SCE.CppInterface.MaterialPreviewManager", "sce"),
    ("SCE.CppInterface.BloodStripManager", "sce"),
    ("SCE.CppInterface.DebugModuleManager", "sce"),
];

var asmMap = new Dictionary<string, Assembly> { ["sce"] = sce, ["scemodule"] = scemodule };

foreach (var (typeName, asmName) in svcServices)
{
    var type = asmMap[asmName].GetType(typeName, throwOnError: false);
    if (type == null)
    {
        Console.WriteLine($"[warn] svc 类型不存在（清单需更新）: {typeName}");
        continue;
    }
    EmitMethods(capabilities, type, asmName, executor: "svc", isStatic: false);
}

// ---------------- cpp.*：SCE.CppInterface 静态方法（全基元/枚举参数 + 非 nint 返回） ----------------
foreach (var type in SafeGetTypes(sce))
{
    if (type.Namespace == null || !type.Namespace.StartsWith("SCE.CppInterface", StringComparison.Ordinal)) continue;
    if (!type.IsClass || !type.IsPublic && !type.IsNestedPublic) continue;
    EmitMethods(capabilities, type, "sce", executor: "cpp", isStatic: true, primitivesOnly: true);
}

// ---------------- 手写静态条目：datacore.* / lua.* / sys.* ----------------
foreach (var e in CatalogGenerator.StaticEntries()) capabilities.Add(e);

// ---------------- 输出 ----------------
var root = new JsonObject
{
    ["engine_version"] = apiVersion,
    ["generated_at"] = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"),
    ["capabilities"] = new JsonArray(capabilities.ToArray<JsonNode?>()),
};

var json = root.ToJsonString(new JsonSerializerOptions
{
    Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    WriteIndented = true,
});
Directory.CreateDirectory(Path.GetDirectoryName(outPath)!);
File.WriteAllText(outPath, json);
Console.WriteLine($"catalog.json 已生成: {outPath}（{capabilities.Count} 条能力，引擎版本 {apiVersion}）");
return 0;

// ---------------- 扫描实现 ----------------

static IEnumerable<Type> SafeGetTypes(Assembly asm)
{
    try { return asm.GetTypes(); }
    catch (ReflectionTypeLoadException ex) { return ex.Types.Where(t => t != null)!; }
}

static void EmitMethods(List<JsonObject> caps, Type type, string asmName, string executor, bool isStatic, bool primitivesOnly = false)
{
    var flags = BindingFlags.Public | BindingFlags.DeclaredOnly | (isStatic ? BindingFlags.Static : BindingFlags.Instance);
    MethodInfo[] methods;
    try { methods = type.GetMethods(flags); }
    catch { return; }

    // 按名字分组处理重载：有重载时 id 生成显式签名后缀 svc.FileSystem.ScanDir(string,bool)
    foreach (var group in methods.GroupBy(m => m.Name))
    {
        var list = group
            .Where(m => !m.IsSpecialName && !m.IsGenericMethodDefinition)
            .Where(m => m.DeclaringType == type) // 排除 object.ToString 等继承方法
            .ToList();
        if (list.Count == 0) continue;
        foreach (var m in list)
        {
            try
            {
                var entry = BuildEntry(type, m, asmName, executor, isStatic, list.Count > 1, primitivesOnly);
                if (entry != null) caps.Add(entry);
            }
            catch
            {
                // 单个方法签名解析失败跳过（泛型参数等）
            }
        }
    }
}

static JsonObject? BuildEntry(Type type, MethodInfo m, string asmName, string executor, bool isStatic, bool overloaded, bool primitivesOnly)
{
    var ps = m.GetParameters();
    // nint/ref/out/指针参数或返回一律不开放（指针透出给 AI 无意义且危险）
    bool unsupported = m.ReturnType == typeof(IntPtr) || m.ReturnType == typeof(UIntPtr) || m.ReturnType.IsPointer
        || ps.Any(p => p.ParameterType == typeof(IntPtr) || p.ParameterType == typeof(UIntPtr)
            || p.ParameterType.IsPointer || p.ParameterType.IsByRef || p.IsOut);
    if (primitivesOnly && unsupported) return null; // cpp 通道直接不收
    if (primitivesOnly && !IsJsonPrimitive(m.ReturnType) && m.ReturnType != typeof(void)) return null;
    if (primitivesOnly && ps.Any(p => !IsJsonPrimitive(p.ParameterType))) return null;

    var id = $"{executor}.{type.Name}.{m.Name}";
    if (overloaded)
    {
        id += "(" + string.Join(",", ps.Select(p => TypeName(p.ParameterType))) + ")";
    }

    var entry = new JsonObject
    {
        ["id"] = id,
        ["executor"] = executor,
        ["type"] = type.FullName,
        ["method"] = m.Name,
        ["assembly"] = asmName,
        ["static"] = isStatic,
        ["returns"] = TypeName(m.ReturnType),
        ["risk"] = GuessRisk(m.Name),
    };
    if (unsupported)
    {
        entry["unsupported"] = true;
        entry["unsupported_reason"] = "签名含 nint/ref/out/指针，不适于 JSON 调用";
    }
    var paramArr = new JsonArray();
    foreach (var p in ps)
    {
        var po = new JsonObject
        {
            ["name"] = p.Name,
            ["type"] = TypeName(p.ParameterType),
            ["required"] = !(p.IsOptional || p.HasDefaultValue),
        };
        if (p.HasDefaultValue && p.DefaultValue is not DBNull && p.DefaultValue != Type.Missing)
        {
            po["default"] = p.DefaultValue switch
            {
                null => null,
                string s => JsonValue.Create(s),
                bool b => JsonValue.Create(b),
                int i => JsonValue.Create(i),
                long l => JsonValue.Create(l),
                double d => JsonValue.Create(d),
                float f => JsonValue.Create(f),
                Enum e => JsonValue.Create(e.ToString()),
                _ => JsonValue.Create(p.DefaultValue.ToString()),
            };
        }
        paramArr.Add(po);
    }
    entry["params"] = paramArr;
    return entry;
}

static bool IsJsonPrimitive(Type t)
{
    t = Nullable.GetUnderlyingType(t) ?? t;
    return t.IsPrimitive || t.IsEnum || t == typeof(string) || t == typeof(decimal);
}

/// <summary>生成期风险预分级：高置信纯查询 read；危险关键词 danger；未知一律 write（不靠前缀猜 read 以外的级别）。</summary>
static string GuessRisk(string methodName)
{
    string[] dangerKeys = ["Exit", "Delete", "SystemCommand", "Publish", "Upload"];
    foreach (var k in dangerKeys)
    {
        if (methodName.Contains(k, StringComparison.OrdinalIgnoreCase)) return "danger";
    }
    string[] readPrefixes = ["Is", "Get", "Query", "Has", "Can", "Scan", "List", "Read", "Contains", "Check", "Count"];
    foreach (var k in readPrefixes)
    {
        if (methodName.StartsWith(k, StringComparison.Ordinal)) return "read";
    }
    // FileExists / DirExists 这类后缀形态
    if (methodName.EndsWith("Exists", StringComparison.Ordinal)) return "read";
    return "write";
}

static string TypeName(Type t)
{
    if (t == typeof(string)) return "string";
    if (t == typeof(bool)) return "bool";
    if (t == typeof(int)) return "int";
    if (t == typeof(uint)) return "uint";
    if (t == typeof(long)) return "long";
    if (t == typeof(float)) return "float";
    if (t == typeof(double)) return "double";
    if (t == typeof(decimal)) return "decimal";
    if (t == typeof(void)) return "void";
    if (t == typeof(object)) return "object";
    if (t.IsEnum) return "enum:" + t.Name;
    if (t.IsGenericType && t.GetGenericTypeDefinition() == typeof(List<>))
        return $"List<{TypeName(t.GetGenericArguments()[0])}>";
    if (t.IsArray) return TypeName(t.GetElementType()!) + "[]";
    if (Nullable.GetUnderlyingType(t) is { } u) return TypeName(u) + "?";
    return t.Name;
}

// 手写静态条目（datacore 高层封装 / lua 桥 method / sys 自描述），生成再多次也不丢。
static class CatalogGenerator
{
    public static IEnumerable<JsonObject> StaticEntries()
    {
        yield return Entry("datacore.list_entries", "datacore", "list_entries", "array",
            "read", []);
        yield return Entry("datacore.read", "datacore", "read", "object",
            "read",
            [
                Param("link", "string", true, null, "数编 entry 完整 link，如 $$.map_config.dflt.root"),
                Param("path", "object", false, null, "可选：字符串点路径或数组段，如 [\"Game\",\"opened_slots\"]"),
            ]);
        yield return Entry("datacore.write", "datacore", "write", "object",
            "write",
            [
                Param("link", "string", true, null, "数编 entry 完整 link"),
                Param("path", "object", true, null, "字段路径（数组段或点路径）"),
                Param("value", "object", true, null, "JSON 值，自动推导 IDataType（bool/int/decimal/string/array/object/null）"),
                Param("auto_commit", "bool", false, JsonValue.Create(true), "默认 true 写完即 CommitChanges；false 则暂存（重开地图丢弃）"),
            ]);
        yield return Entry("datacore.batch_write", "datacore", "batch_write", "object",
            "write",
            [
                Param("changes", "object", true, null, "[{link, path, value}...] 数组，全部 AddGameChange 后只 CommitChanges 一次"),
                Param("auto_commit", "bool", false, JsonValue.Create(true), null),
                Param("on_error", "string", false, JsonValue.Create("abort"), "abort（默认，遇错即断不提交）| commit_partial（提交已应用部分）"),
            ]);
        yield return Entry("lua.list_commands", "lua", "list_commands", "array", "read", []);
        yield return Entry("lua.get_status", "lua", "get_status", "object", "read", []);
        yield return Entry("lua.set_suppress", "lua", "set_suppress", "object", "write",
            [Param("enabled", "bool", true, null, "弹窗抑制开关")]);
        yield return Entry("lua.run_lua", "lua", "run_lua", "object", "danger",
            [Param("code", "string", true, null, "任意 Lua 代码（pcall 执行，兜底逃生舱；danger 级默认放行，可用 config.json danger_deny 显式拒绝）")]);
        yield return Entry("lua.publish_project", "lua", "publish_project", "object", "danger",
            []);
        yield return Entry("lua.capture_game", "lua", "capture_game", "object", "read",
            [Param("path", "string", false, null, "png 落盘绝对路径（缺省自动生成到 用户目录/screenShot/）"), PlayerParam()]);
        yield return Entry("lua.get_game_view_rect", "lua", "get_game_view_rect", "object", "read",
            [PlayerParam()]);
        yield return Entry("lua.find_ui", "lua", "find_ui", "object",
            "read",
            [
                Param("q", "string", false, null, "控件名/id/显示文本子串（模糊、不区分大小写，如 \"商店\" / \"entry\"）；与 kind/scope/tag 至少给一个"),
                Param("kind", "string", false, null, "click=列出全部可点控件 | input=列出全部输入框 | scroll=全部可滚动（不传 q 时用于快速盘点可交互元素）"),
                Param("scope", "string", false, null, "页名前缀过滤（0.8.5 统一 Page 架构：id 路径首段精确匹配，如 \"shop\" 只查 shop 页控件，比 q 快且无歧义）；特殊值 editor=查编辑器自身 UI（base.ui 持久树）"),
                Param("tag", "string[]", false, null, "语义检索标记过滤（0.8.5：Widget props.tag 沉淀进快照；string 或 string[]，任一命中即匹配；返回条目附 tags）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.click_ui", "lua", "click_ui", "object",
            "write",
            [
                Param("id", "string", true, null, "find_ui 返回的控件 id（支持末段简写；仅 clickable=true 的 cgui 控件可点；浮层项等易失效 id 建议改用 lua.tap/lua.pick）"),
                Param("expect", "string", false, null, "操作后验证：延迟数帧断言该文本已出现（注入成功≠业务生效，建议关键操作带上）"),
                Param("expect_absent", "string", false, null, "操作后验证：断言该文本已消失（如关面板）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.click_at", "lua", "click_at", "object",
            "write",
            [
                Param("x", "number", true, null, "游戏视口逻辑坐标 x（与 find_ui rect / capture_game crop 同系）"),
                Param("y", "number", true, null, "游戏视口逻辑坐标 y"),
                PlayerParam(),
            ]);
        yield return Entry("lua.input_text", "lua", "input_text", "object",
            "write",
            [
                Param("id", "string", true, null, "输入框控件 id（find_ui inputable=true 的）"),
                Param("text", "string", true, null, "完整文本（等价人工输入，直接触发 on_input）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.game_info", "lua", "game_info", "object", "read", [PlayerParam()]);
        yield return Entry("lua.press_ui", "lua", "press_ui", "object",
            "write",
            [
                Param("id", "string", true, null, "控件 id（find_ui 返回中 pressable=true 的，如虚拟摇杆）"),
                Param("x", "number", false, null, "按住方向 x（[-1,1]，摇杆用；缺省 0）"),
                Param("y", "number", false, null, "按住方向 y（[-1,1]，向下为正；缺省 0）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.release_ui", "lua", "release_ui", "object",
            "write",
            [Param("id", "string", true, null, "控件 id（解除 press_ui 的模拟按住）"), PlayerParam()]);
        yield return Entry("lua.long_press_ui", "lua", "long_press_ui", "object",
            "write",
            [Param("id", "string", true, null, "控件 id（find_ui 返回中 long_pressable=true 的）"), PlayerParam()]);
        yield return Entry("lua.hover_ui", "lua", "hover_ui", "object",
            "write",
            [Param("id", "string", true, null, "控件 id（真实悬停保持态：虚拟指针驻留，每帧覆写 hover+enter/leave 沿，直到其他虚拟指针命令接管；验证 tooltip/hover 样式配 capture_game crop 判读）"), PlayerParam()]);
        yield return Entry("lua.eval", "lua", "eval", "object", "danger",
            [Param("code", "string", true, null, "游戏 VM（StateGame）内 pcall 执行任意 Lua 代码（游戏侧逃生舱，编辑器侧用 lua.run_lua）"), PlayerParam()]);
        yield return Entry("lua.server_eval", "lua", "server_eval", "object", "danger",
            [Param("code", "string", true, null, "服务端 VM 内 pcall 执行任意 Lua 代码（0.8.10 R6 服务端状态通道：客户端 dbg_bus 转发→服务端 dbg handler 仅 PIE 调试局注册；读服务端权威数据/调物品；require('libs.xxx'/'src.xxx') 源码形态直写）"), PlayerParam()]);
        yield return Entry("lua.drag_ui", "lua", "drag_ui", "object",
            "write",
            [
                Param("from_id", "string", true, null, "拖拽源控件 id"),
                Param("to_id", "string", false, null, "目标控件 id（拖放/排序，落点=目标中心；与 dx/dy 互斥）"),
                Param("dx", "number", false, null, "相对偏移 x 逻辑 px（摇杆/画布用；与 to_id 互斥）"),
                Param("dy", "number", false, null, "相对偏移 y 逻辑 px"),
                PlayerParam(),
            ]);
        yield return Entry("lua.scroll_ui", "lua", "scroll_ui", "object",
            "write",
            [
                Param("id", "string", true, null, "pscroll 容器 id（find_ui 返回中 scrollable=true 的）"),
                Param("delta_y", "number", true, null, "滚动增量逻辑 px（正=向下查看更多内容）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.tap", "lua", "tap", "object",
            "write",
            [
                Param("q", "string", true, null, "控件名/id/显示文本子串（与 find_ui 同语义）：找文本→跟 clickable_ancestor→点击一步完成，多命中取 id 序第一个"),
                Param("expect", "string", false, null, "操作后验证：延迟数帧断言该文本已出现（注入成功≠业务生效，开面板类操作建议带上）"),
                Param("expect_absent", "string", false, null, "操作后验证：断言该文本已消失（如关面板）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.pick", "lua", "pick", "object",
            "write",
            [
                Param("q", "string", true, null, "下拉控件定位（id 或当前选中项文本子串）"),
                Param("item", "string", true, null, "菜单项文本子串（展开+选项一步完成；已展开时幂等）"),
                PlayerParam(),
            ]);
        yield return Entry("lua.key_down", "lua", "key_down", "object",
            "write",
            [Param("key", "string", true, null, "键名（如 \"W\"/\"F1\"，常量见 bgd_const.keyboard）：按下（不调 key_up 则保持按住）"), PlayerParam()]);
        yield return Entry("lua.key_up", "lua", "key_up", "object",
            "write",
            [Param("key", "string", true, null, "键名：松开"), PlayerParam()]);
        yield return Entry("lua.set_value", "lua", "set_value", "object",
            "write",
            [
                Param("id", "string", true, null, "控件 id（find_ui 返回中 settable=true 的 slider）"),
                Param("value", "number", true, null, "目标数值（on_change+on_commit 一次到位，等价拖到位松手）"),
                PlayerParam(),
            ]);
        // 0.8.7 多人调试同族扩展（lua.* 家族新条目，非新 MCP 工具）：玩家暂停/恢复 =
        // 断线/重连模拟（官方 tab 暂停按钮同款 disconnect/reconnect_game_in_editor；单人局无此能力报错）
        yield return Entry("lua.set_pause", "lua", "set_pause", "object",
            "write",
            [
                Param("player", "number", false, null, "目标玩家号（1~4；缺省=多人局 1 号玩家；单人局调用报错「无暂停能力」）"),
                Param("paused", "bool", true, null, "true=暂停（该客户端断线停 tick，dbg 命令不应答/日志无新行/画面定格）；false=恢复（客户端重新连入）"),
            ]);
        yield return Entry("sys.server_info", "sys", "server_info", "object", "read", []);
    }

    private static JsonObject Entry(string id, string executor, string method, string returns, string risk, JsonArray ps)
    {
        return new JsonObject
        {
            ["id"] = id,
            ["executor"] = executor,
            ["method"] = method,
            ["returns"] = returns,
            ["risk"] = risk,
            ["params"] = ps,
        };
    }

    private static JsonObject Param(string name, string type, bool required, JsonNode? def, string? desc)
    {
        var o = new JsonObject
        {
            ["name"] = name,
            ["type"] = type,
            ["required"] = required,
        };
        if (def != null) o["default"] = def;
        if (desc != null) o["description"] = desc;
        return o;
    }

    /// <summary>0.8.7 多人调试统一可选参数：lua.* 游戏侧命令全员可带（缺省=多人局 1 号玩家/单人局唯一玩家）。</summary>
    private static JsonObject PlayerParam() =>
        Param("player", "number", false, null,
            "多人调试定向玩家号（1~4；缺省=多人局 1 号玩家/单人局唯一玩家；单人局带 player 自动回退+告知）");
}
