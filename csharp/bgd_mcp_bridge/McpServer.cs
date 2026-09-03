using System.Diagnostics;
using System.Net;
using System.Reflection;
using System.Text;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace BgdMcpBridge;

/// <summary>
/// 进程内 HTTP + MCP 服务（0.5.0 Gateway 架构）。
/// HttpListener 绑定 127.0.0.1 固定端口（默认 39177，config.json mcp_port 可覆盖），
/// 启动成功把端口号写入 &lt;引擎运行根&gt;/logs/bgd_csharp/port（纯数字）。
/// 端点：POST /rpc（JSON-RPC 风格）、GET /events?since=、POST /mcp（MCP Streamable HTTP）。
///
/// 0.5.0 起放弃「每个能力 = 一个 MCP tool」模型：tools/list 恒定只暴露 ~10 个固定元工具，
/// 全部能力进入可搜索的能力目录（<see cref="CapabilityCatalog"/>），
/// AI 经 search_capabilities → invoke_capability 两步完成调用。
/// 0.4.x 的 list_tool_categories / list_category_tools / cmd_uXXXX slug 机制已废弃删除（无兼容层）。
/// 所有请求处理整体 try-catch，异常返回 JSON error 并记日志。
/// </summary>
public sealed class McpServer
{
    /// <summary>默认监听端口。优先固定不跳变（MCP 客户端通常按 URL 静态配置）；
    /// 但配置端口不可用（被占/落在系统保留段）时自动向后避让——绑不上的端口坚持不跳等于服务永死，
    /// 避让后实际端口写 logs/bgd_csharp/port 文件并记 WARN 日志。</summary>
    public const int PortFixed = 39177;

    /// <summary>MCP 协议版本。</summary>
    public const string ProtocolVersion = "2025-03-26";

    /// <summary>MCP serverInfo.name。</summary>
    public const string ServerName = "bgd_mcp_bridge";

    /// <summary>start_debug 命令超时（启动慢，放宽到 120 秒）。</summary>
    public const int StartDebugTimeoutMs = 120000;

    private readonly EditorBridge _bridge;
    private readonly EventBuffer _events;
    private readonly CapabilityCatalog _catalog;
    private readonly Gateway _gateway;
    private HttpListener? _listener;
    private CancellationTokenSource? _cts;
    private long _requestCount;

    /// <summary>实际绑定端口（0 表示未启动）。</summary>
    public int Port { get; private set; }

    /// <summary>累计处理请求数（供状态页显示）。</summary>
    public long RequestCount => Interlocked.Read(ref _requestCount);

    /// <summary>服务版本（程序集 InformationalVersion，无则 dev）。</summary>
    public static string Version { get; } =
        typeof(McpServer).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "dev";

    public McpServer(EditorBridge bridge, EventBuffer events)
    {
        _bridge = bridge;
        _events = events;
        _catalog = CapabilityCatalog.Load();
        _catalog.PushEvent = (type, data) => _events.Push("bridge", type, data);
        _gateway = new Gateway(_catalog, bridge, events);
    }

    /// <summary>端口文件路径：&lt;引擎运行根&gt;/logs/bgd_csharp/port。无法推导引擎根时返回 null。</summary>
    public static string? GetPortFilePath()
    {
        try
        {
            var root = Logger.TryGetEngineRoot();
            return root == null ? null : Path.Combine(root, "logs", "bgd_csharp", "port");
        }
        catch
        {
            return null;
        }
    }

    /// <summary>解析监听端口：优先读配置文件 &lt;引擎运行根&gt;/logs/bgd_csharp/config.json 的 mcp_port 字段。</summary>
    private static int ResolvePort()
    {
        try
        {
            var root = Logger.TryGetEngineRoot();
            if (root != null)
            {
                var cfgPath = Path.Combine(root, "logs", "bgd_csharp", "config.json");
                if (File.Exists(cfgPath))
                {
                    var doc = JsonNode.Parse(File.ReadAllText(cfgPath)) as JsonObject;
                    var v = doc?["mcp_port"];
                    if (v != null && int.TryParse(v.ToString(), out var p) && p > 1024 && p < 65535)
                    {
                        Logger.Info($"使用配置文件端口: {p}");
                        return p;
                    }
                }
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"读取端口配置失败，用默认端口 {PortFixed}: {ex.Message}");
        }
        return PortFixed;
    }

    // 系统保留 TCP 端口段缓存（启动时探测一次）
    private static List<(int Start, int End)>? _excludedRanges;

    /// <summary>
    /// 探测系统保留 TCP 端口段（netsh int ipv4 show excludedportrange tcp）。
    /// Hyper-V/WSL/winnat 会动态保留整段端口，段内端口任何进程都绑不上，
    /// 报「另一个程序正在使用此文件」(ERROR_SHARING_VIOLATION)，但 netstat 查不到占用进程。
    /// 探测失败静默按无保留段处理（不影响主流程）。
    /// </summary>
    private static List<(int Start, int End)> GetExcludedTcpPortRanges()
    {
        if (_excludedRanges != null) return _excludedRanges;
        var ranges = new List<(int, int)>();
        try
        {
            var psi = new ProcessStartInfo("netsh", "int ipv4 show excludedportrange tcp")
            {
                RedirectStandardOutput = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            using var p = Process.Start(psi);
            if (p != null)
            {
                string output = p.StandardOutput.ReadToEnd();
                p.WaitForExit(5000);
                foreach (var line in output.Split('\n'))
                {
                    // 数据行形如 "     39173       39272      "（可带 * 后缀）；表头/标题行不匹配
                    var m = Regex.Match(line, @"^\s*(\d+)\s+(\d+)\s*\*?\s*$");
                    if (m.Success &&
                        int.TryParse(m.Groups[1].Value, out int s) &&
                        int.TryParse(m.Groups[2].Value, out int e) &&
                        s > 0 && e >= s)
                    {
                        ranges.Add((s, e));
                    }
                }
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"探测系统保留端口段失败（按无保留段处理）: {ex.Message}");
        }
        _excludedRanges = ranges;
        return ranges;
    }

    private static bool IsExcluded(int port, List<(int Start, int End)> ranges)
    {
        foreach (var (s, e) in ranges)
        {
            if (port >= s && port <= e) return true;
        }
        return false;
    }

    /// <summary>启动监听。优先用配置端口；不可用时自动向后避让（跳过系统保留段），实际端口写端口文件并记日志。</summary>
    public bool Start()
    {
        try
        {
            int configured = ResolvePort();
            var excluded = GetExcludedTcpPortRanges();
            if (IsExcluded(configured, excluded))
            {
                // 系统保留段（Hyper-V/WSL/winnat 动态保留）：netstat 查不到占用进程、换段内端口也必失败，极具迷惑性
                Logger.Warn($"端口 {configured} 落在系统保留端口段内（Hyper-V/WSL/winnat 动态保留，netstat 不可见），本次启动自动向后避让。" +
                            "如需固定端口，请改配置到保留段之外（netsh int ipv4 show excludedportrange tcp 查看）。");
            }

            const int MaxProbe = 100;
            for (int i = 0; i < MaxProbe; i++)
            {
                int port = configured + i;
                if (port > 65534) break;
                if (IsExcluded(port, excluded)) continue;
                try
                {
                    var listener = new HttpListener();
                    listener.Prefixes.Add($"http://127.0.0.1:{port}/");
                    listener.Start();
                    _listener = listener;
                    Port = port;
                    if (port != configured)
                    {
                        Logger.Warn($"端口 {configured} 不可用，已自动避让到 {port}。MCP 客户端若按固定 URL 配置请同步修改（实际端口见 logs/bgd_csharp/port 文件）。");
                    }
                    break;
                }
                catch (Exception ex)
                {
                    if (i == 0)
                    {
                        Logger.Warn($"端口 {configured} 绑定失败: {ex.Message}（尝试向后避让）");
                    }
                }
            }

            if (_listener == null)
            {
                Logger.Error($"端口 {configured} 起向后 {MaxProbe} 个候选端口全部不可用，HTTP 服务未启动。" +
                             "请排查占用进程（netstat -ano | findstr <端口>）与系统保留端口段（netsh int ipv4 show excludedportrange tcp）。");
                return false;
            }

            WritePortFile();
            _cts = new CancellationTokenSource();
            _ = Task.Run(AcceptLoopAsync);
            _ = Task.Run(RefreshCommandCapabilitiesAsync);
            Logger.Info($"HTTP 服务已启动: http://127.0.0.1:{Port}/");
            return true;
        }
        catch (Exception ex)
        {
            Logger.Error("HTTP 服务启动失败", ex);
            return false;
        }
    }

    /// <summary>停止监听并删除端口文件（幂等）。仅用于模块主动下线；进程退出场景请用 <see cref="OnProcessExit"/>。</summary>
    public void Stop()
    {
        try { _cts?.Cancel(); } catch { }
        try { if (_listener != null && _listener.IsListening) _listener.Stop(); } catch { }
        DeletePortFile();
    }

    /// <summary>
    /// 进程退出（编辑器关闭）时的清理：只删端口文件，<b>不</b>调用 listener.Stop()（防停机竞态弹框）。
    /// </summary>
    public void OnProcessExit()
    {
        try { _cts?.Cancel(); } catch { }
        DeletePortFile();
    }

    /// <summary>
    /// 从 Lua 侧拉取菜单命令表注入 cmd.* 动态能力条目（能力 ID 即原文，cmd.&lt;命令名&gt;）。
    /// 失败重试几次后放弃（不阻塞服务启动）。
    /// </summary>
    private async Task RefreshCommandCapabilitiesAsync()
    {
        for (int attempt = 1; attempt <= 5; attempt++)
        {
            try
            {
                var data = await _bridge.SendCommandAsync("list_commands", null, 10000).ConfigureAwait(false);
                if (data.HasValue && data.Value.ValueKind == System.Text.Json.JsonValueKind.Array)
                {
                    var names = new List<string>();
                    foreach (var item in data.Value.EnumerateArray())
                    {
                        if (item.ValueKind == System.Text.Json.JsonValueKind.String && item.GetString() is { } s)
                        {
                            names.Add(s);
                        }
                    }
                    if (names.Count > 0)
                    {
                        _catalog.RefreshCommands(names);
                        Logger.Info($"cmd.* 能力条目已注入: {names.Count} 个菜单命令");
                        return;
                    }
                }
            }
            catch (Exception ex)
            {
                Logger.Warn($"拉取菜单命令失败（第 {attempt} 次）: {ex.Message}");
            }
            await Task.Delay(3000).ConfigureAwait(false);
        }
        Logger.Warn("菜单命令拉取失败，cmd.* 能力为空（Lua 侧命令桥未就绪？）");
    }

    private void WritePortFile()
    {
        try
        {
            var path = GetPortFilePath();
            if (path == null) return;
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            File.WriteAllText(path, Port.ToString());
        }
        catch (Exception ex)
        {
            Logger.Warn($"端口文件写入失败: {ex.Message}");
        }
    }

    private static void DeletePortFile()
    {
        try
        {
            var path = GetPortFilePath();
            if (path != null && File.Exists(path)) File.Delete(path);
        }
        catch (Exception ex)
        {
            Logger.Warn($"端口文件删除失败: {ex.Message}");
        }
    }

    private async Task AcceptLoopAsync()
    {
        var listener = _listener;
        if (listener == null) return;
        while (!(_cts?.IsCancellationRequested ?? true))
        {
            HttpListenerContext? ctx = null;
            try
            {
                ctx = await listener.GetContextAsync().ConfigureAwait(false);
            }
            catch (Exception ex) when (ex is HttpListenerException or ObjectDisposedException or OperationCanceledException)
            {
                break; // 停止监听属正常流程
            }
            catch (Exception ex)
            {
                Logger.Error("接受连接异常", ex);
                continue;
            }
            if (ctx == null) continue;
            Interlocked.Increment(ref _requestCount);
            _ = Task.Run(() => HandleSafeAsync(ctx));
        }
    }

    private async Task HandleSafeAsync(HttpListenerContext ctx)
    {
        try
        {
            await HandleAsync(ctx).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            Logger.Error($"请求处理异常: {ctx.Request.HttpMethod} {ctx.Request.Url?.AbsolutePath}", ex);
            try
            {
                await WriteJsonAsync(ctx, 500, JsonOut.Stringify(new JsonObject { ["error"] = ex.Message })).ConfigureAwait(false);
            }
            catch
            {
                // 响应写失败静默
            }
        }
        finally
        {
            try { ctx.Response.OutputStream.Close(); } catch { }
        }
    }

    private async Task HandleAsync(HttpListenerContext ctx)
    {
        var req = ctx.Request;
        var path = req.Url?.AbsolutePath ?? "/";
        var method = req.HttpMethod;

        switch ((method, path))
        {
            case ("POST", "/rpc"):
                await HandleRpcAsync(ctx).ConfigureAwait(false);
                break;
            case ("GET", "/events"):
                await HandleEventsAsync(ctx).ConfigureAwait(false);
                break;
            case ("POST", "/mcp"):
                await HandleMcpAsync(ctx).ConfigureAwait(false);
                break;
            case ("GET", "/mcp"):
                // 不支持 SSE，直接 405
                await WriteJsonAsync(ctx, 405, JsonOut.Stringify(new JsonObject { ["error"] = "SSE not supported" })).ConfigureAwait(false);
                break;
            default:
                await WriteJsonAsync(ctx, 404, JsonOut.Stringify(new JsonObject { ["error"] = "not found" })).ConfigureAwait(false);
                break;
        }
    }

    // ---------------- POST /rpc ----------------

    private async Task HandleRpcAsync(HttpListenerContext ctx)
    {
        var body = await ReadBodyAsync(ctx).ConfigureAwait(false);
        var root = JsonNode.Parse(body) as JsonObject;
        var id = root?["id"];
        var method = JsonRead.Str(root, "method");
        if (root == null || method == null)
        {
            await WriteJsonAsync(ctx, 400, JsonOut.Stringify(new JsonObject { ["id"] = null, ["error"] = "bad request" })).ConfigureAwait(false);
            return;
        }
        var p = root["params"] as JsonObject;
        var r = await DispatchAsync(method, p).ConfigureAwait(false);
        JsonObject resp = r.Ok
            ? new JsonObject { ["id"] = id?.DeepClone(), ["result"] = r.Result }
            : new JsonObject { ["id"] = id?.DeepClone(), ["error"] = r.Error };
        await WriteJsonAsync(ctx, 200, JsonOut.Stringify(resp)).ConfigureAwait(false);
    }

    // ---------------- GET /events?since= ----------------

    private async Task HandleEventsAsync(HttpListenerContext ctx)
    {
        long since = 0;
        var raw = ctx.Request.QueryString["since"];
        if (!string.IsNullOrEmpty(raw)) long.TryParse(raw, out since);

        var (events, latest) = _events.GetSince(since);
        var arr = new JsonArray();
        foreach (var e in events)
        {
            arr.Add(new JsonObject
            {
                ["seq"] = e.Seq,
                ["time"] = e.Time,
                ["source"] = e.Source,
                ["type"] = e.Type,
                ["data"] = e.Data
            });
        }
        var resp = new JsonObject { ["events"] = arr, ["latest"] = latest };
        await WriteJsonAsync(ctx, 200, JsonOut.Stringify(resp)).ConfigureAwait(false);
    }

    // ---------------- POST /mcp（MCP Streamable HTTP） ----------------

    private async Task HandleMcpAsync(HttpListenerContext ctx)
    {
        var body = await ReadBodyAsync(ctx).ConfigureAwait(false);
        var root = JsonNode.Parse(body) as JsonObject;
        var id = root?["id"];
        var method = JsonRead.Str(root, "method");
        if (root == null || method == null)
        {
            await WriteJsonAsync(ctx, 400, McpError(null, -32600, "Invalid Request")).ConfigureAwait(false);
            return;
        }

        switch (method)
        {
            case "initialize":
            {
                var result = new JsonObject
                {
                    ["protocolVersion"] = ProtocolVersion,
                    ["capabilities"] = new JsonObject { ["tools"] = new JsonObject() },
                    ["serverInfo"] = new JsonObject { ["name"] = ServerName, ["version"] = Version }
                };
                await WriteJsonAsync(ctx, 200, McpResult(id, result)).ConfigureAwait(false);
                break;
            }
            case "notifications/initialized":
            {
                ctx.Response.StatusCode = 202;
                break;
            }
            case "ping":
            {
                await WriteJsonAsync(ctx, 200, McpResult(id, new JsonObject())).ConfigureAwait(false);
                break;
            }
            case "tools/list":
            {
                await WriteJsonAsync(ctx, 200, McpResult(id, BuildToolsList())).ConfigureAwait(false);
                break;
            }
            case "tools/call":
            {
                var p = root["params"] as JsonObject;
                var name = JsonRead.Str(p, "name");
                var args = p?["arguments"] as JsonObject;
                if (name == null || !IsMetaTool(name))
                {
                    await WriteJsonAsync(ctx, 200, McpError(id, -32602, name == null ? "missing tool name" : $"unknown tool: {name}（tools/list 恒定只暴露固定元工具，能力调用请走 invoke_capability）")).ConfigureAwait(false);
                    break;
                }
                var r = await DispatchAsync(name, args).ConfigureAwait(false);
                // 出口统一 pretty JSON：中文不转义、无 JSON 套 JSON 双重转义
                JsonObject toolResult = r.Ok
                    ? new JsonObject
                    {
                        ["content"] = new JsonArray(new JsonObject
                        {
                            ["type"] = "text",
                            ["text"] = JsonOut.StringifyPretty(r.Result)
                        })
                    }
                    : new JsonObject
                    {
                        ["content"] = new JsonArray(new JsonObject
                        {
                            ["type"] = "text",
                            ["text"] = JsonOut.StringifyPretty(new JsonObject { ["ok"] = false, ["error"] = r.Error })
                        }),
                        ["isError"] = true
                    };
                await WriteJsonAsync(ctx, 200, McpResult(id, toolResult)).ConfigureAwait(false);
                break;
            }
            default:
            {
                if (method.StartsWith("notifications/", StringComparison.Ordinal))
                {
                    ctx.Response.StatusCode = 202;
                }
                else
                {
                    await WriteJsonAsync(ctx, 200, McpError(id, -32601, "Method not found")).ConfigureAwait(false);
                }
                break;
            }
        }
    }

    private static string McpResult(JsonNode? id, JsonObject result)
    {
        return JsonOut.Stringify(new JsonObject
        {
            ["jsonrpc"] = "2.0",
            ["id"] = id?.DeepClone(),
            ["result"] = result
        });
    }

    private static string McpError(JsonNode? id, int code, string message)
    {
        return JsonOut.Stringify(new JsonObject
        {
            ["jsonrpc"] = "2.0",
            ["id"] = id?.DeepClone(),
            ["error"] = new JsonObject { ["code"] = code, ["message"] = message }
        });
    }

    /// <summary>
    /// 固定元工具集（tools/list 全部内容，永不超过 10 个）。
    /// 能力发现/调用走 search → describe（可选）→ invoke；高频调试操作保留直暴露快捷路径。
    /// </summary>
    private static JsonObject BuildToolsList()
    {
        var tools = JsonNode.Parse("""
        [
          {"name":"search_capabilities","description":"搜索编辑器能力（id/描述/别名/标签模糊匹配）。返回简化签名+风险级别，多数场景 search→invoke 两步完成调用","inputSchema":{"type":"object","properties":{"query":{"type":"string","description":"关键词，可多个（空格分隔，全中优先、无全中自动回退部分命中）"},"limit":{"type":"integer","description":"返回条数，默认 5，上限 10"}},"required":["query"]}},
          {"name":"describe_capability","description":"查看能力完整定义（参数 JSON Schema/返回/风险/示例/前置条件），疑难时深查","inputSchema":{"type":"object","properties":{"id":{"type":"string","description":"能力 id（search 返回的）"}},"required":["id"]}},
          {"name":"invoke_capability","description":"统一调用入口。参数校验失败时错误内嵌 compact schema，按提示修正后重试即可","inputSchema":{"type":"object","properties":{"id":{"type":"string"},"args":{"type":"object","description":"调用参数"},"timeout_ms":{"type":"integer","description":"超时毫秒，默认 5000"}},"required":["id"]}},
          {"name":"list_namespaces","description":"列出能力命名空间（svc/cpp/datacore/cmd/lua/sys）及各空间能力数","inputSchema":{"type":"object","properties":{}}},
          {"name":"get_status","description":"获取编辑器状态（地图/调试中/弹窗抑制）","inputSchema":{"type":"object","properties":{}}},
          {"name":"start_debug","description":"启动调试（需已打开地图）","inputSchema":{"type":"object","properties":{}}},
          {"name":"restart_last_debug","description":"再次调试上次调试版本","inputSchema":{"type":"object","properties":{}}},
          {"name":"stop_debug","description":"停止调试","inputSchema":{"type":"object","properties":{}}},
          {"name":"set_suppress","description":"设置弹窗抑制开关","inputSchema":{"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}},
          {"name":"get_events","description":"拉取事件缓冲中 seq > since 的事件","inputSchema":{"type":"object","properties":{"since":{"type":"integer"}}}}
        ]
        """) as JsonArray;
        return new JsonObject { ["tools"] = tools };
    }

    /// <summary>是否固定元工具。</summary>
    private static bool IsMetaTool(string name) => name is
        "search_capabilities" or "describe_capability" or "invoke_capability" or "list_namespaces"
        or "get_status" or "start_debug" or "restart_last_debug" or "stop_debug"
        or "set_suppress" or "get_events";

    // ---------------- 核心分发（/rpc 与 tools/call 共用） ----------------

    private async Task<OpResult> DispatchAsync(string method, JsonObject? p)
    {
        try
        {
            switch (method)
            {
                case "search_capabilities":
                    return _gateway.Search(p);
                case "describe_capability":
                    return _gateway.Describe(p);
                case "invoke_capability":
                    return await _gateway.InvokeAsync(p).ConfigureAwait(false);
                case "list_namespaces":
                    return _gateway.ListNamespaces();
                case "get_status":
                    return await ViaLuaAsync("get_status", null).ConfigureAwait(false);
                case "set_suppress":
                {
                    bool enabled = JsonRead.Bool(p, "enabled", false);
                    return await ViaLuaAsync("set_suppress", new { enabled }).ConfigureAwait(false);
                }
                case "start_debug":
                {
                    // 防护：地图未就绪时不触发（官方部分调试模块在无图时会崩，如 MutiDebugWindow 依赖 AddMapScoped<IDataCore>）
                    var (sok, sres) = await ViaLuaGetStatusAsync().ConfigureAwait(false);
                    if (sok && string.IsNullOrEmpty(sres?["map_path"]?.GetValue<string>()))
                    {
                        return OpResult.Fail("MAP_NOT_OPEN", "地图未打开：请先在编辑器中打开项目/地图，再启动调试");
                    }
                    // 先经 Lua 桥开弹窗抑制（静默失败不阻断），再直发菜单命令启动调试，最后轮询 get_status 确认 PIE 起来
                    try { await ViaLuaAsync("set_suppress", new { enabled = true }).ConfigureAwait(false); } catch { }
                    _bridge.SendMenuCommand("调试/调试");
                    return await WaitStatusAsync(s => s, StartDebugTimeoutMs, "调试启动超时（PIE 未在预期时间内拉起）").ConfigureAwait(false);
                }
                case "stop_debug":
                {
                    _bridge.SendMenuCommand("调试/停止");
                    return await WaitStatusAsync(s => !s, 15000, "调试停止超时").ConfigureAwait(false);
                }
                case "restart_last_debug":
                {
                    var (sok, sres) = await ViaLuaGetStatusAsync().ConfigureAwait(false);
                    if (sok && string.IsNullOrEmpty(sres?["map_path"]?.GetValue<string>()))
                    {
                        return OpResult.Fail("MAP_NOT_OPEN", "地图未打开：请先在编辑器中打开项目/地图");
                    }
                    try { await ViaLuaAsync("set_suppress", new { enabled = true }).ConfigureAwait(false); } catch { }
                    _bridge.SendMenuCommand("调试/再次调试上次调试版本");
                    return await WaitStatusAsync(s => s, StartDebugTimeoutMs, "再次调试启动超时").ConfigureAwait(false);
                }
                // ---------------- 0.8.7 多人调试：桥内部通道（exe /rpc 直调，不进能力目录） ----------------
                case "mp_start":
                {
                    // 多人拉起（mp_debug.lua：互斥/预校验/字段补全/直调 debug_save_as/逐槽位轮询全在 Lua 侧自持，
                    // 失败原因精确返回）。弹窗抑制由 Lua 侧 auto_suppress 负责。
                    // 超时 160s：Lua 侧最坏 = 15s 停等 + 3s teardown 余量 + 120s 槽位轮询 = 138s，
                    // 必须大于它否则 Lua 的精确报错被本层笼统超时截胡（exe 侧 180s 最外层兜底）
                    return await ViaLuaAsync("mp_start", p, StartDebugTimeoutMs + 40000).ConfigureAwait(false);
                }
                case "mp_switch":
                {
                    // 切焦（capture_game 玩家定向截图的内部编排：切 tab + set_game_ui_focus）
                    return await ViaLuaAsync("mp_switch", p, 10000).ConfigureAwait(false);
                }
                case "mp_logs":
                {
                    // 分玩家日志 tee 查询/清空（get_game_logs player 参数的在线数据源）
                    return await ViaLuaAsync("mp_logs", p, 15000).ConfigureAwait(false);
                }
                case "server_info":
                {
                    return OpResult.Success(new JsonObject
                    {
                        ["version"] = Version,
                        ["port"] = Port,
                        ["engine_version"] = _catalog.EngineVersion,
                        ["catalog_count"] = _catalog.Entries.Count,
                        ["catalog_drifted"] = _catalog.Drifted,
                        ["pid"] = Environment.ProcessId,
                        ["engine_root"] = Logger.TryGetEngineRoot()
                    });
                }
                case "get_events":
                {
                    long since = JsonRead.Long(p, "since", 0);
                    var (events, latest) = _events.GetSince(since);
                    var arr = new JsonArray();
                    foreach (var e in events)
                    {
                        arr.Add(new JsonObject
                        {
                            ["seq"] = e.Seq,
                            ["time"] = e.Time,
                            ["source"] = e.Source,
                            ["type"] = e.Type,
                            ["data"] = e.Data
                        });
                    }
                    return OpResult.Success(new JsonObject { ["events"] = arr, ["latest"] = latest });
                }
                default:
                    return OpResult.Fail("UNKNOWN_METHOD", $"unknown method: {method}",
                        "0.5.0 起能力调用统一走 search_capabilities → invoke_capability；旧 method 已移除");
            }
        }
        catch (Exception ex)
        {
            Logger.Error($"命令分发异常: {method}", ex);
            return OpResult.Fail("INTERNAL", ex.Message, exceptionType: ex.GetType().FullName);
        }
    }

    /// <summary>经 Lua 桥调用并转换结果（ack 的 data 字符串在 C# 侧解析成对象后并入响应，不透出原始转义串）。</summary>
    private async Task<OpResult> ViaLuaAsync(string method, object? paramsObj, int timeoutMs = 30000)
    {
        var r = await _bridge.SendCommandDetailedAsync(method, paramsObj, timeoutMs).ConfigureAwait(false);
        if (!r.Ok) return OpResult.Fail("LUA_ERROR", r.Error ?? "lua error");
        JsonNode? node = null;
        if (r.Data.HasValue)
        {
            try { node = JsonNode.Parse(r.Data.Value.GetRawText()); } catch { }
        }
        return OpResult.Success(node ?? new JsonObject());
    }

    private async Task<(bool Ok, JsonNode? Data)> ViaLuaGetStatusAsync()
    {
        var r = await ViaLuaAsync("get_status", null, 5000).ConfigureAwait(false);
        return (r.Ok, r.Result);
    }

    /// <summary>轮询 get_status 直到 debugging 满足期望值或超时。返回最终的 get_status 数据。</summary>
    private async Task<OpResult> WaitStatusAsync(Func<bool, bool> expect, int timeoutMs, string timeoutError)
    {
        var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);
        JsonNode? last = null;
        while (DateTime.UtcNow < deadline)
        {
            var (ok, result) = await ViaLuaGetStatusAsync().ConfigureAwait(false);
            if (ok && result != null)
            {
                last = result;
                bool debugging = false;
                try { debugging = result["debugging"]?.GetValue<bool>() ?? false; } catch { }
                if (expect(debugging))
                {
                    return OpResult.Success(result);
                }
            }
            await Task.Delay(800).ConfigureAwait(false);
        }
        Logger.Warn($"WaitStatus 超时: {timeoutError}");
        return OpResult.Fail("TIMEOUT", timeoutError);
    }

    // ---------------- 基础 IO ----------------

    private static async Task<string> ReadBodyAsync(HttpListenerContext ctx)
    {
        using var reader = new StreamReader(ctx.Request.InputStream, Encoding.UTF8);
        return await reader.ReadToEndAsync().ConfigureAwait(false);
    }

    private static async Task WriteJsonAsync(HttpListenerContext ctx, int status, string body)
    {
        var resp = ctx.Response;
        resp.StatusCode = status;
        resp.ContentType = "application/json; charset=utf-8";
        var bytes = Encoding.UTF8.GetBytes(body);
        resp.ContentLength64 = bytes.Length;
        await resp.OutputStream.WriteAsync(bytes).ConfigureAwait(false);
    }
}
