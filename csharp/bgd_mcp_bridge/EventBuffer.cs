using NativeEvent = SCEModule.Interface.Event;
using SceApplicationExport = SCE.CppInterface.ApplicationExport;
using SceEventData = SCE.CppInterface.EventData;
using SceEventObject = SCE.CppInterface.EventObject;

namespace BgdMcpBridge;

/// <summary>缓冲事件条目。</summary>
/// <param name="Seq">自增序号。</param>
/// <param name="Time">发生时间（本地时间 HH:mm:ss.fff）。</param>
/// <param name="Source">来源：lua / native。</param>
/// <param name="Type">事件类型（Lua 推送的 type 字段或原生事件名）。</param>
/// <param name="Data">事件数据（JSON 字符串或原文，可为 null）。</param>
public sealed record BridgeEvent(long Seq, string Time, string Source, string Type, string? Data);

/// <summary>
/// 事件缓冲：线程安全定容环形缓冲（容量 1000）。
/// 收集 Lua 推送（bgd_mcp_event）与原生编辑器事件（E_LoadMap / ExitEditor / ShowMessageBox），
/// 供 /events 端点与 MCP get_events 工具拉取。
/// </summary>
public sealed class EventBuffer
{
    /// <summary>缓冲容量。</summary>
    public const int Capacity = 1000;

    private readonly object _lock = new();
    private readonly Queue<BridgeEvent> _queue = new();
    private long _latestSeq;

    /// <summary>收到 ExitEditor 原生事件时回调（用于停止 HTTP 服务并删除端口文件）。</summary>
    public Action? OnExitEditor { get; set; }

    /// <summary>写入一条事件，返回分配的序号。线程安全。</summary>
    public long Push(string source, string type, string? data)
    {
        lock (_lock)
        {
            long seq = ++_latestSeq;
            _queue.Enqueue(new BridgeEvent(seq, DateTime.Now.ToString("HH:mm:ss.fff"), source, type, data));
            while (_queue.Count > Capacity) _queue.Dequeue();
            return seq;
        }
    }

    /// <summary>当前最新序号。</summary>
    public long LatestSeq
    {
        get { lock (_lock) return _latestSeq; }
    }

    /// <summary>取序号大于 since 的全部事件与当前最新序号。</summary>
    public (List<BridgeEvent> Events, long Latest) GetSince(long since)
    {
        lock (_lock)
        {
            var list = new List<BridgeEvent>();
            foreach (var e in _queue)
            {
                if (e.Seq > since) list.Add(e);
            }
            return (list, _latestSeq);
        }
    }

    /// <summary>订阅 Lua 推送事件（bgd_mcp_event）。</summary>
    public void SubscribeLua(SceEventObject eventObject)
    {
        try
        {
            eventObject.SubscribeToEvent(EditorBridge.PushEventName, OnLuaEvent);
        }
        catch (Exception ex)
        {
            Logger.Error($"订阅 {EditorBridge.PushEventName} 失败", ex);
        }
    }

    /// <summary>
    /// 订阅原生编辑器事件（E_LoadMap / ExitEditor / ShowMessageBox）。
    /// Update 太频繁不订。注意：必须在宿主 DI 就绪后调用（ApplicationExport 静态构造依赖 DI）。
    /// </summary>
    public void SubscribeNative()
    {
        try
        {
            SceApplicationExport.SubscribeToEvent(NativeEvent.MapLoaded, _ => Push("native", "E_LoadMap", null));
            SceApplicationExport.SubscribeToEvent(NativeEvent.ShowMessageBox, _ => Push("native", "ShowMessageBox", null));
            SceApplicationExport.SubscribeToEvent(NativeEvent.ExitApplication, OnExitEditorEvent);
        }
        catch (Exception ex)
        {
            Logger.Error("原生事件订阅失败", ex);
        }
    }

    private void OnLuaEvent(SceEventData data)
    {
        try
        {
            string type = "unknown";
            string? payload = null;
            try { if (data.Contains("type")) type = data.GetValue("type").GetString() ?? "unknown"; } catch { }
            try { if (data.Contains("data")) payload = data.GetValue("data").GetString(); } catch { }
            Push("lua", type, payload);
        }
        catch (Exception ex)
        {
            Logger.Error("Lua 推送事件处理异常", ex);
        }
    }

    private void OnExitEditorEvent(SceEventData data)
    {
        try
        {
            Push("native", "ExitEditor", null);
            OnExitEditor?.Invoke();
        }
        catch (Exception ex)
        {
            Logger.Error("ExitEditor 事件处理异常", ex);
        }
    }
}
