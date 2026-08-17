using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using SCEModule.DataCore.Interface;
using SceDataEditor = SCE.CppInterface.DataEditor.DataEditor;
using SceDataLink = SCE.Module.DataCore.DataLink;
using SceEditor = SCEModule.Editor;

namespace BgdMcpBridge;

/// <summary>能力调用统一结果（ok + 结果 / 结构化错误）。</summary>
public sealed record OpResult(bool Ok, JsonNode? Result, JsonObject? Error)
{
    public static OpResult Success(JsonNode? data) => new(true, data, null);

    public static OpResult Fail(string code, string message, string? hint = null, JsonObject? schema = null, string? exceptionType = null)
    {
        var err = new JsonObject
        {
            ["code"] = code,
            ["message"] = message,
        };
        if (hint != null) err["hint"] = hint;
        if (schema != null) err["schema"] = schema;
        if (exceptionType != null) err["exception_type"] = exceptionType;
        return new OpResult(false, null, err);
    }
}

/// <summary>
/// svc.* / cpp.* 反射执行器（0.5.0 M3）。
/// svc：DI 单例（Editor.Current.Services.GetService(Type)）→ 实例方法；
/// cpp：SCE.CppInterface 静态基元方法。
/// 引擎对象一律经 <see cref="UiThreadInvoker"/> 调度到 UI 线程执行并取回结果（带硬超时）；
/// Task/Task&lt;T&gt; 返回值自动 await 解包后再投影序列化。
/// </summary>
public static class ReflectionExecutor
{
    public static async Task<OpResult> InvokeAsync(CapabilityEntry cap, JsonObject? args, int timeoutMs)
    {
        try
        {
            var type = ReflectionUtil.FindType(cap.Type, cap.Assembly);
            if (type == null)
            {
                return OpResult.Fail("UNAVAILABLE", $"当前编辑器版本不可用：类型 {cap.Type} 不存在", "编辑器升级后请重跑生成工具");
            }

            var flags = BindingFlags.Public | (cap.IsStatic ? BindingFlags.Static : BindingFlags.Instance);
            var overloads = type.GetMethods(flags)
                .Where(m => m.Name == cap.Method && !m.IsGenericMethodDefinition)
                .ToList();
            if (overloads.Count == 0)
            {
                return OpResult.Fail("UNAVAILABLE", $"当前编辑器版本不可用：方法 {cap.Type}.{cap.Method} 不存在", "编辑器升级后请重跑生成工具");
            }

            // 重载消歧：catalog id 带显式签名后缀（svc.FileSystem.ScanDir(string,bool)）时按参数类型名匹配
            ArgumentBinder.BindResult bind;
            var sigMatch = MatchBySignature(cap, overloads);
            if (sigMatch != null)
            {
                bind = ArgumentBinder.BindSingle(sigMatch, args);
            }
            else if (overloads.Count == 1)
            {
                bind = ArgumentBinder.BindSingle(overloads[0], args);
            }
            else
            {
                bind = ArgumentBinder.BindOverloads(overloads, args);
            }
            if (!bind.Ok)
            {
                var err = OpResult.Fail("PARAM_INVALID", bind.Error ?? "参数绑定失败",
                    hint: bind.Candidates != null ? "请按候选签名补齐/修正参数" : "按内嵌 schema 修正参数后重试",
                    schema: CompactSchema(cap));
                if (bind.Candidates != null)
                {
                    err.Error!["candidates"] = new JsonArray(bind.Candidates.Select(c => JsonValue.Create(c)).ToArray<JsonNode?>());
                }
                return err;
            }

            // UI 线程执行（引擎对象线程纪律；含默认参数补齐后的完整入参）
            var ui = await UiThreadInvoker.InvokeAsync(() =>
            {
                object? target = null;
                if (!cap.IsStatic)
                {
                    target = SceEditor.Current?.Services?.GetService(type)
                        ?? throw new InvalidOperationException($"DI 服务不可用：{cap.Type}（服务未注册或宿主 DI 未就绪）");
                }
                return bind.Method!.Invoke(target, bind.Args);
            }, timeoutMs).ConfigureAwait(false);

            if (ui.TimedOut)
            {
                return OpResult.Fail("TIMEOUT", $"调用超时（{timeoutMs}ms），疑似模态阻塞", "建议先开启弹窗抑制（set_suppress）后重试");
            }
            if (ui.Error != null)
            {
                var ex = UiThreadInvoker.Unwrap(ui.Error);
                return OpResult.Fail("INVOKE_ERROR", ex.Message, exceptionType: ex.GetType().FullName);
            }

            var value = ui.Value;
            // Task/Task<T> 解包：自动 await（带超时）后再序列化
            if (value is Task task)
            {
                var completed = await Task.WhenAny(task, Task.Delay(timeoutMs)).ConfigureAwait(false);
                if (completed != task)
                {
                    return OpResult.Fail("TIMEOUT", $"异步任务等待超时（{timeoutMs}ms）", "疑似模态阻塞，建议结合弹窗抑制重试");
                }
                try
                {
                    await task.ConfigureAwait(false);
                }
                catch (Exception ex)
                {
                    var real = UiThreadInvoker.Unwrap(ex);
                    return OpResult.Fail("INVOKE_ERROR", real.Message, exceptionType: real.GetType().FullName);
                }
                var tt = task.GetType();
                value = tt.IsGenericType ? tt.GetProperty("Result")?.GetValue(task) : null;
            }

            return OpResult.Success(ReturnProjector.Project(value));
        }
        catch (Exception ex)
        {
            var real = UiThreadInvoker.Unwrap(ex);
            Logger.Error($"反射调用异常 {cap.Id}", real);
            return OpResult.Fail("INVOKE_ERROR", real.Message, exceptionType: real.GetType().FullName);
        }
    }

    /// <summary>catalog id 含显式签名后缀时按参数类型名序列匹配重载。</summary>
    private static MethodInfo? MatchBySignature(CapabilityEntry cap, List<MethodInfo> overloads)
    {
        var idx = cap.Id.IndexOf('(');
        if (idx < 0 || !cap.Id.EndsWith(')')) return null;
        var sig = cap.Id[(idx + 1)..^1];
        var want = sig.Length == 0
            ? Array.Empty<string>()
            : sig.Split(',').Select(s => s.Trim()).ToArray();
        var matched = overloads.Where(m =>
        {
            var ps = m.GetParameters();
            if (ps.Length != want.Length) return false;
            for (int i = 0; i < ps.Length; i++)
            {
                if (!string.Equals(ArgumentBinder.TypeName(ps[i].ParameterType), want[i], StringComparison.OrdinalIgnoreCase)
                    && !string.Equals(ps[i].ParameterType.Name, want[i], StringComparison.OrdinalIgnoreCase))
                {
                    return false;
                }
            }
            return true;
        }).ToList();
        return matched.Count == 1 ? matched[0] : null;
    }

    /// <summary>
    /// 自愈 compact schema（评审 R5 固定契约）：单层 properties、一行 description、
    /// 不展开嵌套对象/$ref（嵌套以「见 describe」占位），控制自愈反馈体积。
    /// </summary>
    public static JsonObject CompactSchema(CapabilityEntry cap)
    {
        var props = new JsonObject();
        var required = new JsonArray();
        foreach (var p in cap.Params)
        {
            props[p.Name] = SchemaOf(p);
            if (p.Required) required.Add(p.Name);
        }
        var schema = new JsonObject
        {
            ["type"] = "object",
            ["properties"] = props,
        };
        if (required.Count > 0) schema["required"] = required;
        return schema;
    }

    private static JsonObject SchemaOf(ParamSpec p)
    {
        var o = new JsonObject { ["type"] = JsonSchemaType(p.Type) };
        if (o["type"]!.GetValue<string>() == "object") o["description"] = "见 describe";
        if (p.Description != null) o["description"] = p.Description;
        if (!p.Required && p.Default != null) o["default"] = p.Default.DeepClone();
        return o;
    }

    /// <summary>catalog 类型串 → JSON Schema 类型。</summary>
    public static string JsonSchemaType(string type)
    {
        return type switch
        {
            "string" => "string",
            "bool" or "boolean" => "boolean",
            "int" or "uint" or "long" or "byte" or "short" => "integer",
            "float" or "double" or "decimal" => "number",
            _ when type.StartsWith("List<", StringComparison.Ordinal) || type.EndsWith("[]", StringComparison.Ordinal) => "array",
            _ when type.StartsWith("enum:", StringComparison.Ordinal) => "string",
            _ => "object",
        };
    }
}

/// <summary>
/// datacore.* 手写高层封装（0.5.0 M4）：IDataCore 数编读写。
/// 不做 DataEditor nint 指针 API 的裸暴露；map-scoped（无图直接返回「地图未打开」，绝不提前触碰 DI）。
/// 批量原子写：全部 AddGameChange 后只调一次 CommitChanges。
/// </summary>
public static class DataCoreExecutor
{
    public static async Task<OpResult> InvokeAsync(CapabilityEntry cap, JsonObject? args, int timeoutMs)
    {
        try
        {
            if (string.IsNullOrEmpty(SceEditor.CurrentMapName))
            {
                return OpResult.Fail("MAP_NOT_OPEN", "地图未打开：datacore 为 map-scoped 能力", "请先在编辑器中打开地图");
            }
            return cap.Method switch
            {
                "list_entries" => await ListEntriesAsync(timeoutMs).ConfigureAwait(false),
                "read" => await ReadAsync(args, timeoutMs).ConfigureAwait(false),
                "write" => await WriteAsync(args, timeoutMs).ConfigureAwait(false),
                "batch_write" => await BatchWriteAsync(args, timeoutMs).ConfigureAwait(false),
                _ => OpResult.Fail("UNKNOWN", $"未知 datacore 能力: {cap.Id}"),
            };
        }
        catch (Exception ex)
        {
            var real = UiThreadInvoker.Unwrap(ex);
            Logger.Error($"datacore 调用异常 {cap.Id}", real);
            return OpResult.Fail("INVOKE_ERROR", real.Message, exceptionType: real.GetType().FullName);
        }
    }

    private static async Task<OpResult> ListEntriesAsync(int timeoutMs)
    {
        var ui = await UiThreadInvoker.InvokeAsync(() =>
        {
            var core = GetCore();
            var arr = new JsonArray();
            foreach (var entry in new[] { core.DefaultGameplayEntry, core.DefaultMapConfigEntry })
            {
                if (entry == null) continue;
                var keys = new JsonArray();
                try
                {
                    foreach (IDataPair pair in entry)
                    {
                        keys.Add(JsonValue.Create(pair.Key?.Value?.ToString()));
                    }
                }
                catch { }
                arr.Add(new JsonObject { ["link"] = entry.FullLink, ["keys"] = keys });
            }
            return (JsonNode?)arr;
        }, timeoutMs).ConfigureAwait(false);
        return UiToOp(ui, timeoutMs);
    }

    private static async Task<OpResult> ReadAsync(JsonObject? args, int timeoutMs)
    {
        var link = JsonRead.Str(args, "link");
        if (string.IsNullOrEmpty(link))
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: link", "提供数编 entry 完整 link，如 $$.map_config.dflt.root");
        }
        var path = ParsePath(args?["path"]);

        var ui = await UiThreadInvoker.InvokeAsync(() =>
        {
            var core = GetCore();
            var entry = core.GetEntryNode(new SceDataLink(link));
            // 业务错误一律返回错误节点、不抛异常：编辑器对 first-chance 异常弹「发生异常」模态并卡死 UI 队列
            if (entry == null) return ErrNode("ENTRY_NOT_FOUND", $"entry 不存在: {link}");
            if (path == null || path.Count == 0)
            {
                return DataObjToJson(entry, 0);
            }
            IDataObj? cur = entry;
            for (int i = 0; i < path.Count; i++)
            {
                if (i == path.Count - 1)
                {
                    if (cur == null || !TryValToJson(cur, path[i], out var v))
                    {
                        return ErrNode("FIELD_NOT_FOUND", $"字段不存在: {path[i]}（路径 {string.Join('.', path.Select(x => x?.ToString()))}）");
                    }
                    return v;
                }
                cur = cur?.TryGet<IDataObj>(path[i] ?? "");
                if (cur == null)
                {
                    return ErrNode("PATH_NOT_FOUND", $"路径段不存在: {string.Join('.', path.Select(x => x?.ToString()))}（止于 {path[i]}）");
                }
            }
            return DataObjToJson(cur, 0);
        }, timeoutMs).ConfigureAwait(false);
        return UiToOp(ui, timeoutMs);
    }

    private static async Task<OpResult> WriteAsync(JsonObject? args, int timeoutMs)
    {
        var link = JsonRead.Str(args, "link");
        if (string.IsNullOrEmpty(link))
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: link");
        }
        var path = StripGamePrefix(ParsePath(args?["path"]));
        if (path == null || path.Count == 0)
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: path", "path 为字符串点路径或数组段，如 [\"Game\",\"opened_slots\"]（相对 Game 对，前缀 \"Game\" 可省）");
        }
        var value = args?["value"];
        // R2 实证结论（2026-08-17）：CommitChanges 落盘不进编辑器 undo 栈，不可 Ctrl+Z 撤销。
        // 因此单写默认不自动提交——AI 必须显式 auto_commit=true；暂存未提交的修改重开地图即丢弃（天然回滚）。
        bool autoCommit = JsonRead.Bool(args, "auto_commit", false);

        var ui = await UiThreadInvoker.InvokeAsync(() =>
        {
            var core = GetCore();
            var data = MakeData(core, value);
            bool ok = core.AddGameChange(link, path, data);
            // 目标字段为小数类型时 MakeInt 可能失败，用 DataDouble 重试一次
            //（不能 fallback MakeDecimal：官方 DataDecimal 直传 decimal 给封送器必抛 InvalidDataException，引擎侧 Bug）
            if (!ok && value != null && value.GetValueKind() == JsonValueKind.Number && ArgumentBinder.TryGet<int>(value, out var intVal))
            {
                ok = core.AddGameChange(link, path, new DataDouble(intVal));
            }
            // 文本（多语言）字段需 MakeText（ENSVM_PASTE + Default 子键），MakeString 直写返回失败
            if (!ok && value != null && value.GetValueKind() == JsonValueKind.String)
            {
                ok = core.AddGameChange(link, path, core.MakeText(value.GetValue<string>()));
            }
            if (!ok)
            {
                return ErrNode("WRITE_FAILED",
                    $"AddGameChange 失败: link={link} path={string.Join('.', path.Select(x => x?.ToString()))}",
                    "path 相对 Game 对（\"Game\" 前缀可省）；数编不允许新建 schema 外字段，只能写既有字段");
            }
            bool committed = false;
            if (autoCommit) committed = core.CommitChanges();
            return (JsonNode?)new JsonObject { ["applied"] = true, ["committed"] = committed };
        }, timeoutMs).ConfigureAwait(false);
        return UiToOp(ui, timeoutMs);
    }

    private static async Task<OpResult> BatchWriteAsync(JsonObject? args, int timeoutMs)
    {
        if (args?["changes"] is not JsonArray changes || changes.Count == 0)
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: changes", "changes 为 [{link, path, value}...] 数组");
        }
        bool autoCommit = JsonRead.Bool(args, "auto_commit", false); // 同 R2 实证：默认不自动提交
        string onError = JsonRead.Str(args, "on_error") ?? "abort";

        var ui = await UiThreadInvoker.InvokeAsync(() =>
        {
            var core = GetCore();
            int applied = 0;
            for (int i = 0; i < changes.Count; i++)
            {
                var c = changes[i] as JsonObject;
                var link = JsonRead.Str(c, "link");
                var path = StripGamePrefix(ParsePath(c?["path"]));
                if (string.IsNullOrEmpty(link) || path == null || path.Count == 0)
                {
                    return BatchFail(i, "change 缺少 link/path", applied, onError == "commit_partial", core);
                }
                var data = MakeData(core, c?["value"]);
                bool ok = core.AddGameChange(link, path, data);
                if (!ok && c?["value"] != null && c["value"]!.GetValueKind() == JsonValueKind.Number && ArgumentBinder.TryGet<int>(c["value"]!, out var intVal))
                {
                    ok = core.AddGameChange(link, path, new DataDouble(intVal));
                }
                if (!ok && c?["value"] != null && c["value"]!.GetValueKind() == JsonValueKind.String)
                {
                    ok = core.AddGameChange(link, path, core.MakeText(c["value"]!.GetValue<string>()));
                }
                if (!ok)
                {
                    return BatchFail(i, "AddGameChange 失败（link 或路径无效）", applied, onError == "commit_partial", core);
                }
                applied++;
            }
            bool committed = false;
            if (autoCommit) committed = core.CommitChanges();
            return (JsonNode?)new JsonObject { ["ok"] = true, ["applied_count"] = applied, ["committed"] = committed };
        }, timeoutMs).ConfigureAwait(false);
        return UiToOp(ui, timeoutMs);
    }

    /// <summary>batch_write 部分失败语义：遇错即断；commit_partial 时提交已应用部分。</summary>
    private static JsonNode BatchFail(int index, string error, int applied, bool commitPartial, IDataCore core)
    {
        bool committed = false;
        if (commitPartial && applied > 0) committed = core.CommitChanges();
        return new JsonObject
        {
            ["ok"] = false,
            ["failed_index"] = index,
            ["error"] = error,
            ["applied_count"] = applied,
            ["committed"] = committed,
            ["hint"] = committed ? "已提交部分修改" : "暂存未提交，重开地图即丢弃",
        };
    }

    private static IDataCore GetCore()
    {
        return SceEditor.GetService<IDataCore>()
            ?? throw new InvalidOperationException("IDataCore 服务不可用");
    }

    /// <summary>
    /// 写路径相对 Game 对（DataCore.AddGameChange 内部固定定位 IsMatchKey("Game") 的 pair，
    /// 官方调用方如 PlayerSettings 传 ["player_setting"] 不带 Game 前缀）。
    /// 为与读路径（含 Game 段）一致，首段为 "Game" 时剥掉。
    /// </summary>
    private static List<object?>? StripGamePrefix(List<object?>? path)
    {
        if (path != null && path.Count > 0 && path[0] is string s && s == "Game")
        {
            path = path.GetRange(1, path.Count - 1);
        }
        return path;
    }

    /// <summary>path 支持字符串点路径与数组段；数字段转 int（List 索引）。</summary>
    private static List<object?>? ParsePath(JsonNode? node)
    {
        if (node == null) return null;
        var list = new List<object?>();
        if (node is JsonArray arr)
        {
            foreach (var seg in arr)
            {
                if (seg == null) { list.Add(null); continue; }
                if (seg.GetValueKind() == JsonValueKind.Number && ArgumentBinder.TryGet<int>(seg, out var i)) list.Add(i);
                else list.Add(seg.GetValueKind() == JsonValueKind.String ? seg.GetValue<string>() : seg.ToString());
            }
            return list;
        }
        if (node.GetValueKind() == JsonValueKind.String)
        {
            foreach (var seg in node.GetValue<string>().Split('.', StringSplitOptions.RemoveEmptyEntries))
            {
                list.Add(int.TryParse(seg, out var i) ? i : seg);
            }
            return list;
        }
        return null;
    }

    /// <summary>JSON → IDataType 递归映射（bool/整数/小数/string/array/object/null）。</summary>
    private static IDataType MakeData(IDataCore core, JsonNode? node)
    {
        if (node == null || node.GetValueKind() == JsonValueKind.Null) return core.MakeNull();
        switch (node.GetValueKind())
        {
            case JsonValueKind.True: return core.MakeBool(true);
            case JsonValueKind.False: return core.MakeBool(false);
            case JsonValueKind.Number:
                if (ArgumentBinder.TryGet<int>(node, out var i)) return core.MakeInt(i);
                // 小数走自实现 DataDouble（官方 MakeDecimal 的封送链必抛 InvalidDataException，见 DataDouble 注释）
                return new DataDouble(node.GetValue<double>());
            case JsonValueKind.String: return core.MakeString(node.GetValue<string>());
            case JsonValueKind.Array:
            {
                var list = new List<IDataType>();
                foreach (var item in (JsonArray)node) list.Add(MakeData(core, item));
                return core.MakeList(list);
            }
            case JsonValueKind.Object:
            {
                var dict = new Dictionary<IDataType, IDataType>();
                foreach (var kv in (JsonObject)node)
                {
                    var key = int.TryParse(kv.Key, out var ki) ? core.MakeInt(ki) : core.MakeString(kv.Key);
                    dict[key] = MakeData(core, kv.Value);
                }
                return core.MakeProp(dict);
            }
            default: return core.MakeNull();
        }
    }

    // ---------------- 读取投影（IDataObj 树 → JSON） ----------------

    private const int ReadMaxDepth = 8;

    /// <summary>
    /// 业务错误节点（ok=false）。<b>禁止</b>用抛异常表达业务错误：编辑器对 first-chance 异常
    /// 记 CSharp\Exception 日志并可能弹「发生异常」模态，模态期间 DispatcherQueue 不泵，全部 UI 调用超时。
    /// UiToOp 识别 ok=false 对象并作为 error 透出。
    /// </summary>
    private static JsonObject ErrNode(string code, string message, string? hint = null)
    {
        var o = new JsonObject { ["ok"] = false, ["code"] = code, ["message"] = message };
        if (hint != null) o["hint"] = hint;
        return o;
    }

    private static bool TryValToJson(IDataObj obj, object? key, out JsonNode? result)
    {
        foreach (IDataPair pair in obj)
        {
            if (KeyEquals(pair.Key?.Value, key))
            {
                var v = pair.Value?.Value;
                result = v is IDataObj child ? DataObjToJson(child, 0) : JsonValue.Create(v?.ToString());
                return true;
            }
        }
        result = null;
        return false;
    }

    private static bool KeyEquals(object? a, object? b)
    {
        if (a == null || b == null) return false;
        if (Equals(a, b)) return true;
        return string.Equals(a.ToString(), b.ToString(), StringComparison.Ordinal);
    }

    private static JsonNode? DataObjToJson(IDataObj obj, int depth)
    {
        if (depth >= ReadMaxDepth) return JsonValue.Create("...(深度截断)");
        var pairs = new List<(object? Key, IDataVal? Val)>();
        try
        {
            foreach (IDataPair pair in obj) pairs.Add((pair.Key?.Value, pair.Value));
        }
        catch (Exception ex)
        {
            return JsonValue.Create($"<枚举失败: {ex.Message}>");
        }
        // 全 int 键且连续 → 数组形态（SCE 数编 list 为 1 基，兼容 0 基）
        bool isList = pairs.Count > 0 && pairs.All(p => p.Key is int)
            && (pairs.Select(p => (int)p.Key!).OrderBy(x => x).SequenceEqual(Enumerable.Range(0, pairs.Count))
                || pairs.Select(p => (int)p.Key!).OrderBy(x => x).SequenceEqual(Enumerable.Range(1, pairs.Count)));
        if (isList)
        {
            var arr = new JsonArray();
            foreach (var p in pairs.OrderBy(p => (int)p.Key!))
            {
                arr.Add(ValNode(p.Val, depth));
            }
            return arr;
        }
        var obj2 = new JsonObject();
        foreach (var p in pairs)
        {
            obj2[p.Key?.ToString() ?? ""] = ValNode(p.Val, depth);
        }
        return obj2;
    }

    private static JsonNode? ValNode(IDataVal? val, int depth)
    {
        var v = val?.Value;
        if (v is IDataObj child) return DataObjToJson(child, depth + 1);
        return v switch
        {
            null => null,
            bool b => JsonValue.Create(b),
            int i => JsonValue.Create(i),
            long l => JsonValue.Create(l),
            float or double or decimal => JsonValue.Create(Convert.ToDouble(v)),
            _ => JsonValue.Create(v.ToString()),
        };
    }

    private static OpResult UiToOp(UiThreadInvoker.UiResult ui, int timeoutMs)
    {
        if (ui.TimedOut)
        {
            return OpResult.Fail("TIMEOUT", $"调用超时（{timeoutMs}ms），疑似模态阻塞", "建议先开启弹窗抑制（set_suppress）后重试");
        }
        if (ui.Error != null)
        {
            var ex = UiThreadInvoker.Unwrap(ui.Error);
            return OpResult.Fail("INVOKE_ERROR", ex.Message, exceptionType: ex.GetType().FullName);
        }
        var value = ui.Value;
        // write 系返回 {ok:false,...} 的业务错误节点作为 error 原样透出（保留 failed_index 等结构）
        if (value is JsonObject o && o["ok"]?.GetValue<bool>() == false)
        {
            return new OpResult(false, null, o);
        }
        return OpResult.Success(value as JsonNode);
    }

    /// <summary>
    /// 自实现小数 IDataType。官方 DataDecimal.ChangeValue 把 decimal 直传 P/Invoke，
    /// 而 DataValMarshaller 封送白名单（bool/short/int/long/float/double/string）不含 decimal，
    /// 必抛 InvalidDataException("Found invalid data while decoding")——引擎侧 Bug。
    /// double 在白名单内（封送为 VT_DECIMAL 兼容），故小数一律走本类。
    /// </summary>
    private sealed class DataDouble(double value) : IDataType
    {
        public object Value => value;

        public bool ChangePair(nint argPtr, object prop)
        {
            return SceDataEditor.DataEditor_PairMake(argPtr, prop, value) != 0;
        }

        public bool ChangeValue(string link, nint ptr, List<object?> path)
        {
            return SceDataEditor.DataEditor_InitEntryNodeSetValueArg(link, ptr, path, SceDataEditor.EnumEntryNodeSetValueMode.ENSVM_NORMAL, value) == 0;
        }
    }
}
