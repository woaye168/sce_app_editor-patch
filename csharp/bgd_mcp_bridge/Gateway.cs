using System.Diagnostics;
using System.Text.Json.Nodes;

namespace BgdMcpBridge;

/// <summary>
/// Gateway 元工具集（0.5.0 M2/M5）：tools/list 恒定只暴露固定元工具，
/// 全部能力经能力目录 search → describe（可选）→ invoke 链路调用。
/// - search 即得简化签名，多数场景 search → invoke 两步完成；
/// - invoke 参数校验失败时错误响应内嵌 compact schema（自愈反馈，AI 下一轮自我修正）；
/// - danger 级能力默认拒绝，需在 config.json danger_allow 显式放行；
/// - write/danger 调用全部进审计日志。
/// </summary>
public sealed class Gateway
{
    private readonly CapabilityCatalog _catalog;
    private readonly EditorBridge _bridge;
    private readonly EventBuffer _events;

    public Gateway(CapabilityCatalog catalog, EditorBridge bridge, EventBuffer events)
    {
        _catalog = catalog;
        _bridge = bridge;
        _events = events;
    }

    // ---------------- search_capabilities ----------------

    public OpResult Search(JsonObject? p)
    {
        var query = JsonRead.Str(p, "query");
        if (string.IsNullOrWhiteSpace(query))
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: query",
                schema: new JsonObject
                {
                    ["type"] = "object",
                    ["properties"] = new JsonObject
                    {
                        ["query"] = new JsonObject { ["type"] = "string", ["description"] = "关键词，可多个（空格分隔，与语义）" },
                        ["limit"] = new JsonObject { ["type"] = "integer", ["default"] = 5, ["description"] = "返回条数，上限 10" },
                    },
                    ["required"] = new JsonArray("query"),
                });
        }
        int limit = JsonRead.Int(p, "limit", 5);
        var (total, results) = _catalog.Search(query, limit);
        var arr = new JsonArray();
        foreach (var (entry, _) in results)
        {
            arr.Add(new JsonObject
            {
                ["id"] = entry.Id,
                ["summary"] = entry.SummaryLine(),
                ["signature"] = entry.CompactSignature(),
                ["risk"] = _catalog.EffectiveRisk(entry),
                ["description"] = entry.Description,
            });
        }
        var result = new JsonObject
        {
            ["ok"] = true,
            ["total_hits"] = total,
            ["results"] = arr,
        };
        var warning = _catalog.SearchHeaderWarning();
        if (warning != null) result["warning"] = warning;
        if (total > results.Count) result["hint"] = "命中过多，请收窄关键词（total_hits 为全部命中数）";
        return OpResult.Success(result);
    }

    // ---------------- describe_capability ----------------

    public OpResult Describe(JsonObject? p)
    {
        var id = JsonRead.Str(p, "id");
        if (string.IsNullOrEmpty(id))
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: id");
        }
        var entry = _catalog.Find(id);
        if (entry == null)
        {
            return OpResult.Fail("NOT_FOUND", $"能力不存在: {id}", "用 search_capabilities 搜索可用能力");
        }
        bool available = _catalog.Validate(entry);
        var result = new JsonObject
        {
            ["ok"] = true,
            ["id"] = entry.Id,
            ["executor"] = entry.Executor,
            ["signature"] = entry.CompactSignature(),
            ["description"] = entry.Description,
            ["risk"] = _catalog.EffectiveRisk(entry),
            ["schema"] = FullSchema(entry),
            ["returns"] = entry.Returns,
            ["precondition"] = entry.Precondition,
            ["example"] = entry.Example,
            ["available"] = available && !entry.Unsupported,
        };
        if (entry.Unsupported) result["unsupported_reason"] = entry.UnsupportedReason ?? "该签名不可开放";
        if (!available) result["unavailable_reason"] = "当前编辑器版本不可用（条目与运行时不匹配），请重跑生成工具";
        if (_catalog.EffectiveRisk(entry) == "danger") result["danger_hint"] = "danger 级默认拒绝，需在 config.json danger_allow 显式放行";
        return OpResult.Success(result);
    }

    /// <summary>完整 JSON Schema（describe 专用，可含默认值/描述；compact 版见 ReflectionExecutor.CompactSchema）。</summary>
    private static JsonObject FullSchema(CapabilityEntry entry)
    {
        var props = new JsonObject();
        var required = new JsonArray();
        foreach (var prm in entry.Params)
        {
            var o = new JsonObject { ["type"] = ReflectionExecutor.JsonSchemaType(prm.Type) };
            if (prm.Description != null) o["description"] = prm.Description;
            if (!prm.Required && prm.Default != null) o["default"] = prm.Default.DeepClone();
            props[prm.Name] = o;
            if (prm.Required) required.Add(prm.Name);
        }
        var schema = new JsonObject { ["type"] = "object", ["properties"] = props };
        if (required.Count > 0) schema["required"] = required;
        return schema;
    }

    // ---------------- invoke_capability ----------------

    public async Task<OpResult> InvokeAsync(JsonObject? p)
    {
        var id = JsonRead.Str(p, "id");
        if (string.IsNullOrEmpty(id))
        {
            return OpResult.Fail("PARAM_INVALID", "Missing required argument: id", "提供能力 id（search_capabilities 返回的）");
        }
        var args = p?["args"] as JsonObject;
        int timeoutMs = JsonRead.Int(p, "timeout_ms", UiThreadInvoker.DefaultTimeoutMs);

        var entry = _catalog.Find(id);
        if (entry == null)
        {
            return OpResult.Fail("NOT_FOUND", $"能力不存在: {id}", "用 search_capabilities 搜索可用能力");
        }
        if (entry.Unsupported)
        {
            return OpResult.Fail("UNSUPPORTED", $"能力不可开放: {id}（{entry.UnsupportedReason ?? "签名不适于 JSON 调用"}）");
        }

        // 安全分级：danger 默认拒绝，需配置文件显式放行
        var risk = _catalog.EffectiveRisk(entry);
        if (risk == "danger" && !IsDangerAllowed(id))
        {
            try { _events.Push("bridge", "danger_denied", id); } catch { }
            AuditLog.Record(id, risk, JsonOut.Stringify(args), 0, "DANGER_DENIED");
            return OpResult.Fail("DANGER_DENIED", $"danger 级能力默认拒绝: {id}",
                "在 <引擎运行根>/logs/bgd_csharp/config.json 的 danger_allow 数组中显式放行该能力 id（或前缀*）后重试");
        }

        // 运行期惰性校验
        if (!_catalog.Validate(entry))
        {
            return OpResult.Fail("UNAVAILABLE", $"当前编辑器版本不可用: {id}", "目录版本漂移，请重跑生成工具");
        }

        var sw = Stopwatch.StartNew();
        OpResult result;
        try
        {
            result = await RouteAsync(entry, args, timeoutMs).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            var real = UiThreadInvoker.Unwrap(ex);
            Logger.Error($"能力调用异常 {id}", real);
            result = OpResult.Fail("INVOKE_ERROR", real.Message, exceptionType: real.GetType().FullName);
        }
        sw.Stop();

        if (risk is "write" or "danger")
        {
            AuditLog.Record(id, risk, JsonOut.Stringify(args), sw.ElapsedMilliseconds,
                result.Ok ? "ok" : (result.Error?["code"]?.GetValue<string>() ?? "fail"));
        }
        return result;
    }

    private async Task<OpResult> RouteAsync(CapabilityEntry entry, JsonObject? args, int timeoutMs)
    {
        switch (entry.Executor)
        {
            case "svc":
            case "cpp":
                return await ReflectionExecutor.InvokeAsync(entry, args, timeoutMs).ConfigureAwait(false);
            case "datacore":
                return await DataCoreExecutor.InvokeAsync(entry, args, timeoutMs).ConfigureAwait(false);
            case "cmd":
            {
                // 直发官方菜单事件（官方菜单点击同款路径，fire-and-forget）
                _bridge.SendMenuCommand(entry.Method!);
                return OpResult.Success(new JsonObject { ["ok"] = true, ["sent"] = entry.Method });
            }
            case "lua":
            {
                var r = await _bridge.SendCommandDetailedAsync(entry.Method!, args, Math.Max(timeoutMs, 15000)).ConfigureAwait(false);
                if (!r.Ok) return OpResult.Fail("LUA_ERROR", r.Error ?? "lua error");
                JsonNode? node = null;
                if (r.Data.HasValue)
                {
                    try { node = JsonNode.Parse(r.Data.Value.GetRawText()); } catch { }
                }
                return OpResult.Success(node ?? new JsonObject());
            }
            case "sys":
                return OpResult.Success(new JsonObject
                {
                    ["ok"] = true,
                    ["version"] = McpServer.Version,
                    ["engine_version"] = _catalog.EngineVersion,
                    ["catalog_count"] = _catalog.Entries.Count,
                    ["catalog_drifted"] = _catalog.Drifted,
                    ["pid"] = Environment.ProcessId,
                    ["engine_root"] = Logger.TryGetEngineRoot(),
                });
            default:
                return OpResult.Fail("UNKNOWN", $"未知执行器通道: {entry.Executor}");
        }
    }

    // ---------------- list_namespaces ----------------

    public OpResult ListNamespaces()
    {
        return OpResult.Success(new JsonObject
        {
            ["ok"] = true,
            ["namespaces"] = _catalog.ListNamespaces(),
        });
    }

    // ---------------- danger 放行清单 ----------------

    /// <summary>读 config.json 的 danger_allow 数组（精确 id 或 前缀* 匹配），每次实时读（小文件，改动即生效）。</summary>
    private static bool IsDangerAllowed(string id)
    {
        try
        {
            var root = Logger.TryGetEngineRoot();
            if (root == null) return false;
            var cfgPath = Path.Combine(root, "logs", "bgd_csharp", "config.json");
            if (!File.Exists(cfgPath)) return false;
            var doc = JsonNode.Parse(File.ReadAllText(cfgPath)) as JsonObject;
            if (doc?["danger_allow"] is not JsonArray allow) return false;
            foreach (var item in allow)
            {
                var s = item?.GetValue<string>();
                if (string.IsNullOrEmpty(s)) continue;
                if (s.EndsWith('*'))
                {
                    if (id.StartsWith(s[..^1], StringComparison.Ordinal)) return true;
                }
                else if (string.Equals(s, id, StringComparison.Ordinal)) return true;
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"danger_allow 读取失败: {ex.Message}");
        }
        return false;
    }
}
