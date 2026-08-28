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
    outPath = args.Length > 2 ? args[2] : Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "bgd_mcp_bridge", "catalog.json"));
    Console.WriteLine($"已从项目推导：hostDir={hostDir} api={apiVersion}");
}
else if (args.Length >= 2)
{
    hostDir = args[0];
    apiVersion = args[1];
    outPath = args.Length > 2 ? args[2] : Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "bgd_mcp_bridge", "catalog.json"));
}
else
{
    Console.Error.WriteLine("用法: make_catalog --project <项目根> [<输出路径>]  |  make_catalog <宿主目录> <api版本> [<输出路径>]");
    return 1;
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
    // 0.8.0 起剔除 SCE.CppInterface.FileSystem：文件读删/复制/改时间等 AI 用自身工具即可完成，
    // 不需要 MCP 通道（减少搜索噪音）
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
            [Param("code", "string", true, null, "任意 Lua 代码（pcall 执行，兜底逃生舱；默认 danger 需配置放行）")]);
        yield return Entry("lua.publish_project", "lua", "publish_project", "object", "danger",
            []);
        yield return Entry("lua.capture_game", "lua", "capture_game", "object", "read",
            [Param("path", "string", false, null, "png 落盘绝对路径（缺省自动生成到 用户目录/screenShot/）")]);
        yield return Entry("lua.get_game_view_rect", "lua", "get_game_view_rect", "object", "read", []);
        yield return Entry("lua.find_ui", "lua", "find_ui", "object",
            "read",
            [
                Param("q", "string", false, null, "控件名/id/显示文本子串（模糊、不区分大小写，如 \"商店\" / \"entry\"）；与 kind 至少给一个"),
                Param("kind", "string", false, null, "click=列出全部可点控件 | input=列出全部输入框（不传 q 时用于快速盘点可交互元素）"),
                Param("scope", "string", false, JsonValue.Create("game"), "game=游戏UI（cgui 快照+base.ui 树，默认）| editor=编辑器 UI（base.ui 持久树）"),
            ]);
        yield return Entry("lua.click_ui", "lua", "click_ui", "object",
            "write",
            [Param("id", "string", true, null, "find_ui 返回的控件 id（支持末段简写；仅 clickable=true 的 cgui 控件可点）")]);
        yield return Entry("lua.click_at", "lua", "click_at", "object",
            "write",
            [
                Param("x", "number", true, null, "游戏视口逻辑坐标 x（与 find_ui rect / capture_game crop 同系）"),
                Param("y", "number", true, null, "游戏视口逻辑坐标 y"),
            ]);
        yield return Entry("lua.input_text", "lua", "input_text", "object",
            "write",
            [
                Param("id", "string", true, null, "输入框控件 id（find_ui inputable=true 的）"),
                Param("text", "string", true, null, "完整文本（等价人工输入，直接触发 on_input）"),
            ]);
        yield return Entry("lua.game_info", "lua", "game_info", "object", "read", []);
        yield return Entry("lua.press_ui", "lua", "press_ui", "object",
            "write",
            [
                Param("id", "string", true, null, "控件 id（find_ui 返回中 pressable=true 的，如虚拟摇杆）"),
                Param("x", "number", false, null, "按住方向 x（[-1,1]，摇杆用；缺省 0）"),
                Param("y", "number", false, null, "按住方向 y（[-1,1]，向下为正；缺省 0）"),
            ]);
        yield return Entry("lua.release_ui", "lua", "release_ui", "object",
            "write",
            [Param("id", "string", true, null, "控件 id（解除 press_ui 的模拟按住）")]);
        yield return Entry("lua.long_press_ui", "lua", "long_press_ui", "object",
            "write",
            [Param("id", "string", true, null, "控件 id（find_ui 返回中 long_pressable=true 的）")]);
        yield return Entry("lua.hover_ui", "lua", "hover_ui", "object",
            "read",
            [Param("id", "string", true, null, "控件 id（hover 暂无法模拟，调用返回原因说明与替代方案）")]);
        yield return Entry("lua.set_value", "lua", "set_value", "object",
            "write",
            [
                Param("id", "string", true, null, "控件 id（find_ui 返回中 settable=true 的 slider）"),
                Param("value", "number", true, null, "目标数值（on_change+on_commit 一次到位，等价拖到位松手）"),
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
}
