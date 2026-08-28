using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace BgdMcpBridge;

/// <summary>能力参数规格（catalog 骨架 + 签名展示/参数校验用）。</summary>
public sealed class ParamSpec
{
    public string Name { get; set; } = "";
    public string Type { get; set; } = "object";
    public bool Required { get; set; } = true;
    public JsonNode? Default { get; set; }
    public string? Description { get; set; }
}

/// <summary>
/// 能力条目。能力 ID 即原文（cmd.调试/调试、svc.FileSystem.ScanDir），
/// 走 invoke 的 JSON 字符串参数传递，不受 MCP tool name 字符集限制。
/// </summary>
public sealed class CapabilityEntry
{
    public string Id { get; set; } = "";
    /// <summary>执行器路由前缀：svc / cpp / datacore / cmd / lua / sys。</summary>
    public string Executor { get; set; } = "";
    /// <summary>目标类型全名（svc/cpp 通道）。</summary>
    public string? Type { get; set; }
    /// <summary>目标方法名（svc/cpp）或 Lua 桥 method（lua）或菜单命令名（cmd）。</summary>
    public string? Method { get; set; }
    /// <summary>程序集来源（svc/cpp，如 sce / scemodule）。</summary>
    public string? Assembly { get; set; }
    public bool IsStatic { get; set; }
    public List<ParamSpec> Params { get; set; } = new();
    public string? Returns { get; set; }

    // ---- annotations.json 合并层（人工标注，仓库长期资产） ----
    public string? Description { get; set; }
    public List<string> Aliases { get; set; } = new();
    public List<string> Tags { get; set; } = new();
    /// <summary>风险级别：read / write / danger。</summary>
    public string Risk { get; set; } = "write";
    public string? Example { get; set; }
    public string? Precondition { get; set; }

    // ---- 运行时状态 ----
    /// <summary>nint 指针/不可序列化签名等不可开放条目标记。</summary>
    public bool Unsupported { get; set; }
    public string? UnsupportedReason { get; set; }

    /// <summary>简化签名（search 一行摘要 / 自愈 compact schema 用）。</summary>
    public string CompactSignature()
    {
        var ps = string.Join(", ", Params.Select(p =>
            p.Required ? $"{p.Name}: {p.Type}" : $"{p.Name}: {p.Type} = {(p.Default?.ToJsonString(JsonOut.Options) ?? "null")}"));
        return $"{Method ?? Id}({ps})" + (string.IsNullOrEmpty(Returns) ? "" : $" → {Returns}");
    }

    /// <summary>一行摘要：id — 中文描述 [risk]。</summary>
    public string SummaryLine()
    {
        var desc = string.IsNullOrEmpty(Description) ? "" : $" — {Description}";
        return $"{Id}{desc} [{Risk}]";
    }
}

/// <summary>
/// 能力目录（0.5.0 Gateway 核心）。catalog.json 构建期生成并编译期嵌入 dll；
/// annotations.json 人工标注层（中文描述/别名/标签/风险/服务准入/黑名单）。
/// 运行期惰性校验 + 版本漂移主动通知（/events + search 头部标注）。
/// </summary>
public sealed class CapabilityCatalog
{
    /// <summary>catalog 头记录的引擎版本（生成工具写入，如 "13"）。</summary>
    public string EngineVersion { get; private set; } = "";

    private readonly List<CapabilityEntry> _entries = new();
    private readonly Dictionary<string, CapabilityEntry> _byId = new(StringComparer.Ordinal);
    private readonly HashSet<string> _admittedServices = new(StringComparer.Ordinal);
    private readonly HashSet<string> _blacklist = new(StringComparer.Ordinal);

    /// <summary>惰性校验结果缓存：id → true 可用 / false 当前编辑器版本不可用。</summary>
    private readonly Dictionary<string, bool> _validation = new(StringComparer.Ordinal);

    /// <summary>版本漂移持久 warning 状态。</summary>
    public bool Drifted { get; private set; }

    private bool _driftEventPushed;

    /// <summary>漂移/审计事件推送通道（McpServer 注入，推到 /events）。</summary>
    public Action<string, string?>? PushEvent { get; set; }

    public IReadOnlyList<CapabilityEntry> Entries => _entries;

    /// <summary>从嵌入资源加载 catalog.json + annotations.json 并合并。任何失败静默降级（空目录）。</summary>
    public static CapabilityCatalog Load()
    {
        var cat = new CapabilityCatalog();
        try
        {
            var catalogJson = ReadEmbeddedResource("catalog.json");
            if (catalogJson != null) cat.LoadCatalog(catalogJson);
            var annotationsJson = ReadEmbeddedResource("annotations.json");
            if (annotationsJson != null) cat.ApplyAnnotations(annotationsJson);
            cat.CheckVersionDrift();
            Logger.Info($"能力目录已加载: {cat._entries.Count} 条（准入服务 {cat._admittedServices.Count} 个，引擎版本 {cat.EngineVersion}，漂移={cat.Drifted}）");
        }
        catch (Exception ex)
        {
            Logger.Error("能力目录加载失败", ex);
        }
        return cat;
    }

    private static string? ReadEmbeddedResource(string logicalName)
    {
        try
        {
            var asm = typeof(CapabilityCatalog).Assembly;
            var name = asm.GetManifestResourceNames()
                .FirstOrDefault(n => n.Equals(logicalName, StringComparison.Ordinal) || n.EndsWith("." + logicalName, StringComparison.Ordinal));
            if (name == null) return null;
            using var stream = asm.GetManifestResourceStream(name);
            if (stream == null) return null;
            using var reader = new StreamReader(stream);
            return reader.ReadToEnd();
        }
        catch (Exception ex)
        {
            Logger.Warn($"嵌入资源读取失败 {logicalName}: {ex.Message}");
            return null;
        }
    }

    private void LoadCatalog(string json)
    {
        var root = JsonNode.Parse(json) as JsonObject;
        if (root == null) return;
        EngineVersion = root["engine_version"]?.GetValue<string>() ?? "";
        var caps = root["capabilities"] as JsonArray;
        if (caps == null) return;
        foreach (var node in caps)
        {
            if (node is not JsonObject o) continue;
            var entry = ParseEntry(o);
            if (entry == null) continue;
            AddEntry(entry);
        }
    }

    private static CapabilityEntry? ParseEntry(JsonObject o)
    {
        try
        {
            var id = o["id"]?.GetValue<string>();
            if (string.IsNullOrEmpty(id)) return null;
            var entry = new CapabilityEntry
            {
                Id = id,
                Executor = o["executor"]?.GetValue<string>() ?? (id.Contains('.') ? id[..id.IndexOf('.')] : ""),
                Type = o["type"]?.GetValue<string>(),
                Method = o["method"]?.GetValue<string>(),
                Assembly = o["assembly"]?.GetValue<string>(),
                IsStatic = o["static"]?.GetValue<bool>() ?? false,
                Returns = o["returns"]?.GetValue<string>(),
                Unsupported = o["unsupported"]?.GetValue<bool>() ?? false,
                UnsupportedReason = o["unsupported_reason"]?.GetValue<string>(),
            };
            if (o["risk"]?.GetValue<string>() is { } genRisk) entry.Risk = genRisk;
            if (o["params"] is JsonArray ps)
            {
                foreach (var p in ps)
                {
                    if (p is not JsonObject po) continue;
                    entry.Params.Add(new ParamSpec
                    {
                        Name = po["name"]?.GetValue<string>() ?? "",
                        Type = po["type"]?.GetValue<string>() ?? "object",
                        Required = po["required"]?.GetValue<bool>() ?? true,
                        Default = po["default"]?.DeepClone(),
                        Description = po["description"]?.GetValue<string>(),
                    });
                }
            }
            return entry;
        }
        catch (Exception ex)
        {
            Logger.Warn($"catalog 条目解析失败: {ex.Message}");
            return null;
        }
    }

    private void AddEntry(CapabilityEntry entry)
    {
        if (_byId.ContainsKey(entry.Id)) return;
        _entries.Add(entry);
        _byId[entry.Id] = entry;
    }

    private void ApplyAnnotations(string json)
    {
        var root = JsonNode.Parse(json) as JsonObject;
        if (root == null) return;

        if (root["services"] is JsonObject services)
        {
            foreach (var kv in services)
            {
                if (kv.Value is JsonObject so && (so["admitted"]?.GetValue<bool>() ?? false))
                {
                    _admittedServices.Add(kv.Key);
                }
            }
        }
        if (root["blacklist"] is JsonArray bl)
        {
            foreach (var b in bl)
            {
                if (b?.GetValue<string>() is { } s) _blacklist.Add(s);
            }
        }
        if (root["entries"] is JsonObject entries)
        {
            foreach (var kv in entries)
            {
                if (!_byId.TryGetValue(kv.Key, out var entry) || kv.Value is not JsonObject eo) continue;
                entry.Description = eo["description"]?.GetValue<string>() ?? entry.Description;
                entry.Example = eo["example"]?.GetValue<string>() ?? entry.Example;
                entry.Precondition = eo["precondition"]?.GetValue<string>() ?? entry.Precondition;
                if (eo["risk"]?.GetValue<string>() is { } risk) entry.Risk = risk;
                if (eo["aliases"] is JsonArray aliases)
                {
                    foreach (var a in aliases) { if (a?.GetValue<string>() is { } s) entry.Aliases.Add(s); }
                }
                if (eo["tags"] is JsonArray tags)
                {
                    foreach (var t in tags) { if (t?.GetValue<string>() is { } s) entry.Tags.Add(s); }
                }
            }
        }
    }

    /// <summary>
    /// 运行时注入 cmd.* 动态条目（Lua 侧 list_commands 返回的菜单命令）。
    /// 能力 ID 即原文（cmd.&lt;命令名&gt;），重复调用幂等。
    /// </summary>
    public void RefreshCommands(IEnumerable<string> commandNames)
    {
        foreach (var name in commandNames)
        {
            if (string.IsNullOrEmpty(name)) continue;
            var id = "cmd." + name;
            if (_byId.ContainsKey(id)) continue;
            AddEntry(new CapabilityEntry
            {
                Id = id,
                Executor = "cmd",
                Method = name,
                Description = "菜单命令",
                Risk = RiskOf(id),
            });
        }
    }

    // ---------------- 安全分级 ----------------

    /// <summary>判定条目风险级别（人工标注优先；未标注按规则推定，未知一律 write）。</summary>
    private string RiskOf(string id)
    {
        if (IsBlacklisted(id)) return "danger";
        // danger 关键词（Exit/Delete/SystemCommand/Publish/Upload + 中文对应）
        string[] dangerKeys = { "Exit", "Delete", "SystemCommand", "Publish", "Upload", "发布", "上传", "退出", "删除" };
        foreach (var k in dangerKeys)
        {
            if (id.Contains(k, StringComparison.OrdinalIgnoreCase)) return "danger";
        }
        return "write";
    }

    /// <summary>黑名单命中（支持 * 后缀前缀匹配）。</summary>
    public bool IsBlacklisted(string id)
    {
        foreach (var b in _blacklist)
        {
            if (b.EndsWith('*'))
            {
                if (id.StartsWith(b[..^1], StringComparison.Ordinal)) return true;
            }
            else if (string.Equals(b, id, StringComparison.Ordinal)) return true;
        }
        return false;
    }

    /// <summary>svc 服务是否已准入（服务级准入制：默认整体不开放，annotations 逐服务人工准入）。</summary>
    public bool IsServiceAdmitted(CapabilityEntry entry)
    {
        if (entry.Executor != "svc") return true;
        return entry.Type != null && _admittedServices.Contains(entry.Type);
    }

    /// <summary>条目最终风险级别：未准入 svc / 黑名单一律 danger。</summary>
    public string EffectiveRisk(CapabilityEntry entry)
    {
        if (IsBlacklisted(entry.Id)) return "danger";
        if (!IsServiceAdmitted(entry)) return "danger";
        return entry.Risk;
    }

    // ---------------- 搜索 / 描述 ----------------

    /// <summary>
    /// 关键词搜索（id/描述/别名/标签模糊匹配）。
    /// limit 默认 5、上限 10；打分链：精确 id &gt; 别名 &gt; 标签 &gt; 描述，同分按 id 长度升序。
    /// 多关键词为空格分隔「与」语义；**0.8.1 起全中优先、无全中时回退部分命中**
    /// （严格 AND 对自然语言多词查询过于苛刻，实测「找控件 点击按钮 定位」零命中），
    /// 回退时按 命中词数 &gt; 总分 排序并置 PartialFallback=true 供上层提示。
    /// 返回（命中数, 结果条目含分数, 是否部分命中回退）。
    /// </summary>
    public (int TotalHits, List<(CapabilityEntry Entry, int Score)> Results, bool PartialFallback) Search(string query, int limit = 5)
    {
        if (limit <= 0) limit = 5;
        if (limit > 10) limit = 10;
        var tokens = query.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (tokens.Length == 0) return (0, new(), false);

        var allHit = new List<(CapabilityEntry, int)>();
        var partial = new List<(CapabilityEntry, int Matched, int Score)>();
        foreach (var entry in _entries)
        {
            // 未准入 svc 不进 search（服务级准入制）；unsupported 条目不开放
            if (!IsServiceAdmitted(entry) || entry.Unsupported) continue;
            int total = 0;
            int matched = 0;
            foreach (var token in tokens)
            {
                int s = ScoreToken(entry, token);
                if (s > 0) { matched++; total += s; }
            }
            if (matched == tokens.Length) allHit.Add((entry, total));
            else if (matched > 0) partial.Add((entry, matched, total));
        }
        allHit.Sort((a, b) =>
        {
            int c = b.Item2.CompareTo(a.Item2);
            return c != 0 ? c : a.Item1.Id.Length.CompareTo(b.Item1.Id.Length);
        });
        if (allHit.Count > 0) return (allHit.Count, allHit.Take(limit).ToList(), false);
        // 回退：部分命中按 命中词数 > 总分 排序
        partial.Sort((a, b) =>
        {
            int c = b.Matched.CompareTo(a.Matched);
            if (c != 0) return c;
            c = b.Score.CompareTo(a.Score);
            return c != 0 ? c : a.Item1.Id.Length.CompareTo(b.Item1.Id.Length);
        });
        return (partial.Count, partial.Take(limit).Select(p => (p.Item1, p.Score)).ToList(), true);
    }

    private static int ScoreToken(CapabilityEntry entry, string token)
    {
        if (string.Equals(entry.Id, token, StringComparison.OrdinalIgnoreCase)) return 100;
        foreach (var a in entry.Aliases)
        {
            if (string.Equals(a, token, StringComparison.OrdinalIgnoreCase)) return 80;
        }
        if (entry.Id.Contains(token, StringComparison.OrdinalIgnoreCase)) return 40;
        foreach (var a in entry.Aliases)
        {
            if (a.Contains(token, StringComparison.OrdinalIgnoreCase)) return 30;
        }
        foreach (var t in entry.Tags)
        {
            if (t.Contains(token, StringComparison.OrdinalIgnoreCase)) return 25;
        }
        if (entry.Description != null && entry.Description.Contains(token, StringComparison.OrdinalIgnoreCase)) return 10;
        return 0;
    }

    public CapabilityEntry? Find(string id)
    {
        return _byId.TryGetValue(id, out var e) ? e : null;
    }

    /// <summary>按命名空间（执行器前缀）统计能力数。</summary>
    public JsonArray ListNamespaces()
    {
        var arr = new JsonArray();
        foreach (var g in _entries.GroupBy(e => e.Executor).OrderBy(g => g.Key, StringComparer.Ordinal))
        {
            arr.Add(new JsonObject
            {
                ["namespace"] = g.Key,
                ["count"] = g.Count(),
                ["searchable"] = g.Count(e => IsServiceAdmitted(e) && !e.Unsupported),
            });
        }
        return arr;
    }

    // ---------------- 运行期惰性校验 + 版本漂移 ----------------

    /// <summary>
    /// 运行期惰性校验：条目首次 describe/invoke 时反射确认存在且签名匹配。
    /// 不匹配置版本漂移 warning 并返回 false（「当前编辑器版本不可用」）。
    /// </summary>
    public bool Validate(CapabilityEntry entry)
    {
        if (_validation.TryGetValue(entry.Id, out var ok)) return ok;
        ok = ValidateCore(entry);
        _validation[entry.Id] = ok;
        if (!ok)
        {
            MarkDrift($"条目 {entry.Id} 与当前编辑器版本不匹配");
        }
        return ok;
    }

    private bool ValidateCore(CapabilityEntry entry)
    {
        try
        {
            switch (entry.Executor)
            {
                case "svc":
                case "cpp":
                {
                    var type = ReflectionUtil.FindType(entry.Type, entry.Assembly);
                    if (type == null) return false;
                    var methods = type.GetMethods(BindingFlags.Public | (entry.IsStatic ? BindingFlags.Static : BindingFlags.Instance))
                        .Where(m => m.Name == entry.Method)
                        .ToList();
                    if (methods.Count == 0) return false;
                    // 有重载签名后缀的条目按参数个数粗匹配
                    if (entry.Params.Count > 0)
                    {
                        return methods.Any(m => m.GetParameters().Length >= entry.Params.Count(p => p.Required));
                    }
                    return true;
                }
                default:
                    return true; // cmd/lua/datacore/sys 由各自通道运行期自证
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"条目校验异常 {entry.Id}: {ex.Message}");
            return false;
        }
    }

    /// <summary>启动时比对 catalog 引擎版本与当前编辑器版本（exe 位于 version-XX 目录）。</summary>
    private void CheckVersionDrift()
    {
        try
        {
            var exe = System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName;
            var dir = exe != null ? Path.GetFileName(Path.GetDirectoryName(exe)) : null;
            // version-13 → 13
            var runtime = dir != null && dir.StartsWith("version-", StringComparison.OrdinalIgnoreCase) ? dir["version-".Length..] : null;
            if (!string.IsNullOrEmpty(EngineVersion) && !string.IsNullOrEmpty(runtime) &&
                !string.Equals(EngineVersion, runtime, StringComparison.OrdinalIgnoreCase))
            {
                MarkDrift($"catalog 引擎版本({EngineVersion}) ≠ 当前编辑器版本({runtime})");
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"版本漂移检测失败: {ex.Message}");
        }
    }

    private void MarkDrift(string reason)
    {
        if (!Drifted)
        {
            Drifted = true;
            Logger.Warn($"能力目录版本漂移: {reason}，请重跑生成工具");
        }
        if (!_driftEventPushed)
        {
            _driftEventPushed = true;
            try { PushEvent?.Invoke("catalog_version_mismatch", reason); } catch { }
        }
    }

    /// <summary>search 结果头部固定标注（漂移时）。</summary>
    public string? SearchHeaderWarning()
    {
        return Drifted ? "目录版本漂移，部分能力可能不可用，请重跑生成工具" : null;
    }
}

/// <summary>反射辅助：按全名+程序集名找类型（宿主进程内 sce/scemodule 已加载）。</summary>
public static class ReflectionUtil
{
    public static Type? FindType(string? fullName, string? assemblyName)
    {
        if (string.IsNullOrEmpty(fullName)) return null;
        try
        {
            if (!string.IsNullOrEmpty(assemblyName))
            {
                var t = Type.GetType($"{fullName}, {assemblyName}", throwOnError: false);
                if (t != null) return t;
            }
            var direct = Type.GetType(fullName, throwOnError: false);
            if (direct != null) return direct;
            foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
            {
                Type? t = null;
                try { t = asm.GetType(fullName, throwOnError: false); } catch { }
                if (t != null) return t;
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"类型查找失败 {fullName}: {ex.Message}");
        }
        return null;
    }
}
