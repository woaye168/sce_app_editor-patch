using System.Net;
using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace BgdMcpBridge;

/// <summary>
/// 进程内 HTTP + MCP 服务。
/// HttpListener 绑定 127.0.0.1，端口 39177 起被占则 +1 重试至 39187；全部失败记日志不抛。
/// 启动成功把端口号写入 <![CDATA[<引擎运行根>/logs/bgd_csharp/port]]>（纯数字）。
/// 端点：POST /rpc（JSON-RPC 风格）、GET /events?since=、POST /mcp（MCP Streamable HTTP）。
/// 所有请求处理整体 try-catch，异常返回 JSON error 并记日志。
/// </summary>
public sealed class McpServer
{
    /// <summary>端口区间起点。</summary>
    public const int PortStart = 39177;

    /// <summary>端口区间终点（含）。</summary>
    public const int PortEnd = 39187;

    /// <summary>MCP 协议版本。</summary>
    public const string ProtocolVersion = "2025-03-26";

    /// <summary>MCP serverInfo.name。</summary>
    public const string ServerName = "bgd_mcp_bridge";

    /// <summary>start_debug 命令超时（启动慢，放宽到 120 秒）。</summary>
    public const int StartDebugTimeoutMs = 120000;

    private readonly EditorBridge _bridge;
    private readonly EventBuffer _events;
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
    }

    /// <summary>端口文件路径：<引擎运行根>/logs/bgd_csharp/port。无法推导引擎根时返回 null。</summary>
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

    /// <summary>启动监听。成功返回 true；失败记日志返回 false（不抛异常）。</summary>
    public bool Start()
    {
        try
        {
            for (int port = PortStart; port <= PortEnd; port++)
            {
                try
                {
                    var listener = new HttpListener();
                    listener.Prefixes.Add($"http://127.0.0.1:{port}/");
                    listener.Start();
                    _listener = listener;
                    Port = port;
                    break;
                }
                catch (Exception ex)
                {
                    Logger.Warn($"端口 {port} 绑定失败: {ex.Message}");
                }
            }

            if (_listener == null)
            {
                Logger.Error($"端口区间 {PortStart}-{PortEnd} 全部被占，HTTP 服务未启动");
                return false;
            }

            WritePortFile();
            _cts = new CancellationTokenSource();
            _ = Task.Run(AcceptLoopAsync);
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
    /// 进程退出（编辑器关闭）时的清理：只删端口文件，<b>不</b>调用 listener.Stop()。
    /// 原因：ExitApplication 事件在引擎事件线程触发，此时 Stop 会 dispose 底层
    /// HttpRequestQueueV2Handle，正阻塞在 GetContextAsync 的 IO 线程随即抛出
    /// HttpListenerException(995)/ObjectDisposedException 并逃逸为未处理异常（弹原生框）。
    /// 进程即将退出，监听器交给 OS 收尸即可，无需显式停止。
    /// </summary>
    public void OnProcessExit()
    {
        try { _cts?.Cancel(); } catch { }
        DeletePortFile();
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
                await WriteJsonAsync(ctx, 500, new JsonObject
                {
                    ["error"] = ex.Message
                }.ToJsonString()).ConfigureAwait(false);
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
                // 首版不支持 SSE，直接 405
                await WriteJsonAsync(ctx, 405, new JsonObject { ["error"] = "SSE not supported" }.ToJsonString()).ConfigureAwait(false);
                break;
            default:
                await WriteJsonAsync(ctx, 404, new JsonObject { ["error"] = "not found" }.ToJsonString()).ConfigureAwait(false);
                break;
        }
    }

    // ---------------- POST /rpc ----------------

    private async Task HandleRpcAsync(HttpListenerContext ctx)
    {
        var body = await ReadBodyAsync(ctx).ConfigureAwait(false);
        var root = JsonNode.Parse(body) as JsonObject;
        var id = root?["id"];
        var method = root?["method"]?.GetValue<string>();
        if (root == null || method == null)
        {
            await WriteJsonAsync(ctx, 400, new JsonObject { ["id"] = null, ["error"] = "bad request" }.ToJsonString()).ConfigureAwait(false);
            return;
        }
        var p = root["params"] as JsonObject;
        var (ok, result, error) = await DispatchAsync(method, p).ConfigureAwait(false);
        JsonObject resp = ok
            ? new JsonObject { ["id"] = id?.DeepClone(), ["result"] = result }
            : new JsonObject { ["id"] = id?.DeepClone(), ["error"] = error };
        await WriteJsonAsync(ctx, 200, resp.ToJsonString()).ConfigureAwait(false);
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
        await WriteJsonAsync(ctx, 200, resp.ToJsonString()).ConfigureAwait(false);
    }

    // ---------------- POST /mcp（MCP Streamable HTTP） ----------------

    private async Task HandleMcpAsync(HttpListenerContext ctx)
    {
        var body = await ReadBodyAsync(ctx).ConfigureAwait(false);
        var root = JsonNode.Parse(body) as JsonObject;
        var id = root?["id"];
        var method = root?["method"]?.GetValue<string>();
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
                // 通知无响应体
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
                var name = p?["name"]?.GetValue<string>();
                var args = p?["arguments"] as JsonObject;
                if (name == null)
                {
                    await WriteJsonAsync(ctx, 200, McpError(id, -32602, "missing tool name")).ConfigureAwait(false);
                    break;
                }
                // 动态命令 tool（slug）→ 反查原始命令名，走 call_command 通道
                string? dynamicCmd = null;
                if (!IsBuiltinTool(name))
                {
                    dynamicCmd = ResolveCommandTool(name);
                    if (dynamicCmd == null)
                    {
                        await WriteJsonAsync(ctx, 200, McpError(id, -32602, $"unknown tool: {name}")).ConfigureAwait(false);
                        break;
                    }
                }
                var (ok, result, error) = dynamicCmd != null
                    ? await DispatchAsync("call_command", new JsonObject { ["name"] = dynamicCmd }).ConfigureAwait(false)
                    : await DispatchAsync(name, args).ConfigureAwait(false);
                JsonObject toolResult = ok
                    ? new JsonObject
                    {
                        ["content"] = new JsonArray(new JsonObject
                        {
                            ["type"] = "text",
                            ["text"] = result?.ToJsonString() ?? "{}"
                        })
                    }
                    : new JsonObject
                    {
                        ["content"] = new JsonArray(new JsonObject
                        {
                            ["type"] = "text",
                            ["text"] = error ?? "unknown error"
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
        return new JsonObject
        {
            ["jsonrpc"] = "2.0",
            ["id"] = id?.DeepClone(),
            ["result"] = result
        }.ToJsonString();
    }

    private static string McpError(JsonNode? id, int code, string message)
    {
        return new JsonObject
        {
            ["jsonrpc"] = "2.0",
            ["id"] = id?.DeepClone(),
            ["error"] = new JsonObject { ["code"] = code, ["message"] = message }
        }.ToJsonString();
    }

    // 动态命令 tools：slug → 原始命令名 的映射缓存（list_commands 拉取一次后缓存）
    private readonly Dictionary<string, string> _commandToolMap = new();
    private bool _commandToolsLoaded;

    private JsonObject BuildToolsList()
    {
        // 固定工具清单（inputSchema 为 JSON Schema）
        var tools = JsonNode.Parse("""
        [
          {"name":"call_command","description":"调用编辑器 Lua 侧注册的命令","inputSchema":{"type":"object","properties":{"name":{"type":"string","description":"命令名"}},"required":["name"]}},
          {"name":"list_commands","description":"列出 Lua 侧全部可用命令","inputSchema":{"type":"object","properties":{}}},
          {"name":"get_status","description":"获取编辑器状态","inputSchema":{"type":"object","properties":{}}},
          {"name":"start_debug","description":"启动调试（调试/调试）","inputSchema":{"type":"object","properties":{}}},
          {"name":"stop_debug","description":"停止调试（调试/停止）","inputSchema":{"type":"object","properties":{}}},
          {"name":"set_suppress","description":"设置弹窗抑制开关","inputSchema":{"type":"object","properties":{"enabled":{"type":"boolean"}},"required":["enabled"]}},
          {"name":"get_events","description":"拉取事件缓冲中 seq > since 的事件","inputSchema":{"type":"object","properties":{"since":{"type":"integer"}}}}
        ]
        """) as JsonArray;

        // 动态展开：把 Lua 侧每个已注册命令暴露为一个独立 tool（tool 名用安全 slug，中文命令入 description）
        foreach (var (slug, cmdName) in GetCommandToolMap())
        {
            tools!.Add(new JsonObject
            {
                ["name"] = slug,
                ["description"] = $"编辑器命令：{cmdName}",
                ["inputSchema"] = new JsonObject { ["type"] = "object", ["properties"] = new JsonObject() }
            });
        }
        return new JsonObject { ["tools"] = tools };
    }

    /// <summary>拉取（并缓存）Lua 命令 → MCP 安全 tool 名的映射。失败返回已缓存或空。</summary>
    private IReadOnlyDictionary<string, string> GetCommandToolMap()
    {
        if (_commandToolsLoaded) return _commandToolMap;
        _commandToolsLoaded = true;
        try
        {
            var data = _bridge.SendCommandAsync("list_commands", null, 10000)
                .ConfigureAwait(false).GetAwaiter().GetResult();
            if (data.HasValue && data.Value.ValueKind == JsonValueKind.Array)
            {
                var seen = new HashSet<string>();
                foreach (var item in data.Value.EnumerateArray())
                {
                    var cmd = item.ValueKind == JsonValueKind.String ? item.GetString() : null;
                    if (string.IsNullOrEmpty(cmd)) continue;
                    var slug = ToToolSlug(cmd, seen);
                    if (slug != null) _commandToolMap[slug] = cmd;
                }
                Logger.Info($"动态命令 tools 已加载: {_commandToolMap.Count} 个");
            }
        }
        catch (Exception ex)
        {
            Logger.Warn($"加载动态命令 tools 失败: {ex.Message}");
        }
        return _commandToolMap;
    }

    /// <summary>把中文/含斜杠命令名转成 MCP 合法 tool 名（[a-zA-Z0-9_-]，≤64）。</summary>
    private static string? ToToolSlug(string cmd, HashSet<string> seen)
    {
        var sb = new System.Text.StringBuilder("cmd_");
        foreach (var ch in cmd)
        {
            if (char.IsAsciiLetterOrDigit(ch)) sb.Append(ch);
            else if (ch == '/' || ch == '(' || ch == ')' || ch == ' ' || ch == '-') sb.Append('_');
            else sb.Append('u').Append(((int)ch).ToString("x4")); // 中文等非 ASCII → uXXXX
        }
        var slug = sb.ToString();
        if (slug.Length > 64) slug = slug[..64];
        if (!seen.Add(slug)) return null; // 冲突则跳过（极端情况）
        return slug;
    }

    /// <summary>是否内置固定 tool。</summary>
    private static bool IsBuiltinTool(string name) => name is
        "call_command" or "list_commands" or "get_status" or "start_debug"
        or "stop_debug" or "set_suppress" or "get_events";

    /// <summary>若 tool 名是动态命令 slug，返回原始命令名；否则 null。</summary>
    private string? ResolveCommandTool(string toolName)
    {
        return GetCommandToolMap().TryGetValue(toolName, out var cmd) ? cmd : null;
    }

    // ---------------- 核心分发（/rpc 与 tools/call 共用） ----------------

    private async Task<(bool Ok, JsonNode? Result, string? Error)> DispatchAsync(string method, JsonObject? p)
    {
        try
        {
            switch (method)
            {
                case "call_command":
                {
                    var name = p?["name"]?.GetValue<string>();
                    if (string.IsNullOrEmpty(name)) return (false, null, "missing params.name");
                    // 直发官方菜单事件（官方菜单点击同款路径，fire-and-forget）
                    _bridge.SendMenuCommand(name);
                    return (true, new JsonObject { ["sent"] = name }, null);
                }
                case "list_commands":
                    return await ViaLuaAsync("list_commands", null).ConfigureAwait(false);
                case "get_status":
                    return await ViaLuaAsync("get_status", null).ConfigureAwait(false);
                case "set_suppress":
                {
                    bool enabled = p?["enabled"]?.GetValue<bool>() ?? false;
                    return await ViaLuaAsync("set_suppress", new { enabled }).ConfigureAwait(false);
                }
                case "start_debug":
                {
                    // 防护：地图未就绪时不触发（官方部分调试模块在无图时会崩，如 MutiDebugWindow 依赖 AddMapScoped<IDataCore>）
                    var (sok, sres, _) = await ViaLuaAsync("get_status", null, 5000).ConfigureAwait(false);
                    if (sok && sres != null)
                    {
                        var mapPath = sres["map_path"]?.GetValue<string>();
                        if (string.IsNullOrEmpty(mapPath))
                        {
                            return (false, null, "地图未打开：请先在编辑器中打开项目/地图，再启动调试");
                        }
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
                case "server_info":
                {
                    return (true, new JsonObject
                    {
                        ["version"] = Version,
                        ["port"] = Port,
                        ["pid"] = Environment.ProcessId,
                        ["engine_root"] = Logger.TryGetEngineRoot()
                    }, null);
                }
                case "get_events":
                {
                    long since = 0;
                    try { since = p?["since"]?.GetValue<long>() ?? 0; } catch { }
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
                    return (true, new JsonObject { ["events"] = arr, ["latest"] = latest }, null);
                }
                default:
                    return (false, null, $"unknown method: {method}");
            }
        }
        catch (Exception ex)
        {
            Logger.Error($"命令分发异常: {method}", ex);
            return (false, null, ex.Message);
        }
    }

    /// <summary>经 Lua 桥调用并转换结果。</summary>
    private async Task<(bool Ok, JsonNode? Result, string? Error)> ViaLuaAsync(string method, object? paramsObj, int timeoutMs = 30000)
    {
        var r = await _bridge.SendCommandDetailedAsync(method, paramsObj, timeoutMs).ConfigureAwait(false);
        if (!r.Ok) return (false, null, r.Error ?? "lua error");
        JsonNode? node = null;
        if (r.Data.HasValue)
        {
            try { node = JsonNode.Parse(r.Data.Value.GetRawText()); } catch { }
        }
        return (true, node ?? new JsonObject(), null);
    }

    /// <summary>
    /// 轮询 get_status 直到 debugging 满足期望值或超时。
    /// expect 接收当前 debugging 布尔值，返回 true 表示达成。返回最终的 get_status 数据。
    /// </summary>
    private async Task<(bool Ok, JsonNode? Result, string? Error)> WaitStatusAsync(Func<bool, bool> expect, int timeoutMs, string timeoutError)
    {
        var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);
        JsonNode? last = null;
        while (DateTime.UtcNow < deadline)
        {
            var (ok, result, _) = await ViaLuaAsync("get_status", null, 5000).ConfigureAwait(false);
            if (ok && result != null)
            {
                last = result;
                bool debugging = false;
                try { debugging = result["debugging"]?.GetValue<bool>() ?? false; } catch { }
                if (expect(debugging))
                {
                    return (true, result, null);
                }
            }
            await Task.Delay(800).ConfigureAwait(false);
        }
        Logger.Warn($"WaitStatus 超时: {timeoutError}");
        return (false, last, timeoutError);
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
