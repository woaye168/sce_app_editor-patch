using System.Net;
using System.Reflection;
using System.Text;
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

    /// <summary>停止监听并删除端口文件（幂等）。</summary>
    public void Stop()
    {
        try { _cts?.Cancel(); } catch { }
        try { _listener?.Stop(); } catch { }
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
                var (ok, result, error) = await DispatchAsync(name, args).ConfigureAwait(false);
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

    private static JsonObject BuildToolsList()
    {
        // 工具清单（inputSchema 为 JSON Schema）
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
        return new JsonObject { ["tools"] = tools };
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
                    return await ViaLuaAsync("call_command", new { name }).ConfigureAwait(false);
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
                    return await ViaLuaAsync("call_command", new { name = "调试/调试" }, StartDebugTimeoutMs).ConfigureAwait(false);
                case "stop_debug":
                    return await ViaLuaAsync("call_command", new { name = "调试/停止" }).ConfigureAwait(false);
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
