using System.Collections.Concurrent;
using System.Text.Json;
using SceEventData = SCE.CppInterface.EventData;
using SceEventManager = SCE.CppInterface.EventManager;
using SceEventObject = SCE.CppInterface.EventObject;

namespace BgdMcpBridge;

/// <summary>Lua 命令桥调用结果。</summary>
/// <param name="Ok">Lua 侧是否成功（ack 的 ok 字段）。</param>
/// <param name="Data">成功时的数据（ack 的 data 字段，JSON 字符串解析后的元素）。</param>
/// <param name="Error">失败原因（Lua 侧 error 字段或本侧超时/异常）。</param>
public sealed record BridgeResult(bool Ok, JsonElement? Data, string? Error);

/// <summary>
/// Lua 命令桥：C# → Lua 发送命令事件（bgd_mcp_cmd），等待 Lua 回执（bgd_mcp_ack）。
/// 命令载荷：{id, method, params}；回执载荷：{id, ok, data} 或 {id, ok=false, error}。
/// 所有宿主交互均 try-catch，异常只记日志绝不外抛。
/// </summary>
public sealed class EditorBridge
{
    /// <summary>C# → Lua 命令事件名。</summary>
    public const string CmdEventName = "bgd_mcp_cmd";

    /// <summary>Lua → C# 回执事件名。</summary>
    public const string AckEventName = "bgd_mcp_ack";

    /// <summary>Lua → C# 事件推送事件名（弹窗抑制等）。</summary>
    public const string PushEventName = "bgd_mcp_event";

    private readonly SceEventManager _eventManager;
    private long _nextId;

    // 等待中的请求：id → 完成源
    private readonly ConcurrentDictionary<long, TaskCompletionSource<BridgeResult>> _pending = new();

    public EditorBridge(SceEventManager eventManager, SceEventObject eventObject)
    {
        _eventManager = eventManager;
        try
        {
            eventObject.SubscribeToEvent(AckEventName, OnAck);
        }
        catch (Exception ex)
        {
            Logger.Error($"订阅 {AckEventName} 失败", ex);
        }
    }

    /// <summary>
    /// 发送 Lua 命令并等待回执。超时/失败不抛异常，返回 Ok=false 的结果。
    /// </summary>
    public async Task<BridgeResult> SendCommandDetailedAsync(string method, object? paramsObj, int timeoutMs = 30000)
    {
        long id = Interlocked.Increment(ref _nextId);
        var tcs = new TaskCompletionSource<BridgeResult>(TaskCreationOptions.RunContinuationsAsynchronously);
        _pending[id] = tcs;
        try
        {
            var payload = JsonOut.Serialize(new { id, method, @params = paramsObj });
            SendOnUiThread(payload);

            var timeoutTask = Task.Delay(timeoutMs);
            var completed = await Task.WhenAny(tcs.Task, timeoutTask).ConfigureAwait(false);
            if (completed != tcs.Task)
            {
                Logger.Warn($"Lua 命令超时: method={method}, id={id}, timeout={timeoutMs}ms");
                return new BridgeResult(false, null, $"timeout({timeoutMs}ms)");
            }
            return await tcs.Task.ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            Logger.Error($"Lua 命令发送失败: method={method}, id={id}", ex);
            return new BridgeResult(false, null, ex.Message);
        }
        finally
        {
            _pending.TryRemove(id, out _);
        }
    }

    /// <summary>
    /// 发送 Lua 命令，仅返回数据部分（失败返回 null 并记日志）。
    /// </summary>
    public async Task<JsonElement?> SendCommandAsync(string method, object? paramsObj, int timeoutMs = 30000)
    {
        var result = await SendCommandDetailedAsync(method, paramsObj, timeoutMs).ConfigureAwait(false);
        return result.Ok ? result.Data : null;
    }

    /// <summary>切到 UI 线程发送命令事件（引擎事件 API 需在 UI 线程调用）。</summary>
    private void SendOnUiThread(string json)
    {
        try
        {
            var dq = BridgeWindow.Instance?.DispatcherQueue;
            if (dq != null && !dq.HasThreadAccess)
            {
                if (!dq.TryEnqueue(() => SendDirect(json)))
                    Logger.Warn("UI 线程投递失败（TryEnqueue 返回 false）");
            }
            else
            {
                SendDirect(json);
            }
        }
        catch (Exception ex)
        {
            Logger.Error("命令事件发送异常", ex);
        }
    }

    private void SendDirect(string json)
    {
        try
        {
            _eventManager.SendEvent(CmdEventName, json);
        }
        catch (Exception ex)
        {
            Logger.Error($"SendEvent({CmdEventName}) 失败", ex);
        }
    }

    /// <summary>
    /// 直发官方菜单命令：对原生事件 'EditorMainTitleMenuBar' SendEvent（命令名字符串）。
    /// 这是官方菜单点击的精确路径（menu_bar.lua:1066 register_event 收到后 call_command(name)），
    /// 不依赖 Lua 侧 require ui.menu_bar 的时机/形态，是 call_command 的可靠通道。
    /// 原生事件发送是 fire-and-forget（无回执），发送成功不代表命令执行成功。
    /// </summary>
    public void SendMenuCommand(string commandName)
    {
        try
        {
            var dq = BridgeWindow.Instance?.DispatcherQueue;
            void Send()
            {
                try { _eventManager.SendEvent("EditorMainTitleMenuBar", commandName); }
                catch (Exception ex) { Logger.Error("SendEvent(EditorMainTitleMenuBar) 失败", ex); }
            }
            if (dq != null && !dq.HasThreadAccess)
            {
                if (!dq.TryEnqueue(Send)) Logger.Warn("UI 线程投递失败（菜单命令）");
            }
            else
            {
                Send();
            }
        }
        catch (Exception ex)
        {
            Logger.Error("菜单命令发送异常", ex);
        }
    }

    /// <summary>回执事件处理（引擎事件线程触发，绝不抛出异常）。</summary>
    private void OnAck(SceEventData data)
    {
        try
        {
            if (!data.Contains("id")) return;
            long id = ReadLong(data, "id");
            if (!_pending.TryRemove(id, out var tcs)) return; // 已超时的迟到回执直接丢弃

            bool ok = ReadBool(data, "ok");
            string? payload = ReadString(data, "data");
            string? error = ReadString(data, "error");

            JsonElement? parsed = null;
            if (ok)
            {
                // 成功路径：Lua 的 data 字段是原生 Lua 表，经 VariantMap 传为 StringVector（JSON 数组文本），按 JSON 解析
                if (!string.IsNullOrEmpty(payload))
                {
                    try
                    {
                        // Clone 使元素脱离临时 JsonDocument 生命周期
                        parsed = JsonDocument.Parse(payload).RootElement.Clone();
                    }
                    catch (Exception ex)
                    {
                        Logger.Warn($"ack data 非合法 JSON，按原文忽略: {ex.Message}");
                    }
                }
            }
            tcs.TrySetResult(new BridgeResult(ok, parsed, ok ? null : (error ?? "lua error")));
        }
        catch (Exception ex)
        {
            Logger.Error("ack 事件处理异常", ex);
        }
    }

    // ---- EventData 字段读取辅助：逐个 try，字段类型不符不炸整个处理 ----

    private static long ReadLong(SceEventData data, string key)
    {
        try { return data.GetValue(key).GetInt(); } catch { }
        try { return long.Parse(data.GetValue(key).GetString()); } catch { }
        return 0;
    }

    private static bool ReadBool(SceEventData data, string key)
    {
        if (!data.Contains(key)) return false;
        try { return data.GetValue(key).GetBool(); } catch { }
        try { return data.GetValue(key).GetInt() != 0; } catch { }
        try { return data.GetValue(key).GetString() is "true" or "1"; } catch { }
        return false;
    }

    private static string? ReadString(SceEventData data, string key)
    {
        if (!data.Contains(key)) return null;
        try { return data.GetValue(key).GetString(); } catch { }
        // Lua 表经引擎序列化为 StringVector（如 JSON 数组文本），StringVector 转 string 为空，需兜底
        try
        {
            var vec = data.GetValue(key).GetStringVector();
            if (vec is { Count: > 0 }) return string.Join('\n', vec);
        }
        catch { }
        return null;
    }
}
