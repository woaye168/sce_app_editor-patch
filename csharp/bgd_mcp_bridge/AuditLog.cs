using System.Threading.Channels;

namespace BgdMcpBridge;

/// <summary>
/// 审计日志（0.5.0）：所有 write/danger 能力调用的「能力 id + 入参 + 耗时 + 结果/异常」
/// 异步写 &lt;引擎运行根&gt;/logs/bgd_csharp/audit-YYYY-MM-DD.log。
/// 实现：Channel（有界容量 1000 + TryWrite 丢弃）+ 后台单线程消费写盘——
/// 审计丢了可补日志，绝不能文件句柄竞争反压阻塞 Gateway 主流程。
/// </summary>
public static class AuditLog
{
    private sealed record AuditEntry(string Line);

    private static readonly Channel<AuditEntry> _channel =
        Channel.CreateBounded<AuditEntry>(new BoundedChannelOptions(1000)
        {
            SingleReader = true,
            SingleWriter = false,
            FullMode = BoundedChannelFullMode.DropWrite,
        });

    private static int _started;

    /// <summary>记录一条审计（write/danger 调用）。非阻塞，队列满直接丢弃。</summary>
    public static void Record(string capabilityId, string risk, string? argsJson, long elapsedMs, string outcome)
    {
        try
        {
            EnsureStarted();
            var line = $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] [{risk}] {capabilityId} elapsed={elapsedMs}ms outcome={outcome} args={(argsJson ?? "{}")}";
            _channel.Writer.TryWrite(new AuditEntry(line));
        }
        catch
        {
            // 审计失败不影响主流程
        }
    }

    private static void EnsureStarted()
    {
        if (Interlocked.Exchange(ref _started, 1) == 1) return;
        try
        {
            _ = Task.Run(ConsumeLoopAsync);
        }
        catch (Exception ex)
        {
            Logger.Warn($"审计消费线程启动失败: {ex.Message}");
        }
    }

    private static async Task ConsumeLoopAsync()
    {
        await foreach (var entry in _channel.Reader.ReadAllAsync())
        {
            try
            {
                var root = Logger.TryGetEngineRoot();
                if (root == null) continue;
                var dir = Path.Combine(root, "logs", "bgd_csharp");
                Directory.CreateDirectory(dir);
                var file = Path.Combine(dir, $"audit-{DateTime.Now:yyyy-MM-dd}.log");
                File.AppendAllText(file, entry.Line + Environment.NewLine);
            }
            catch
            {
                // 写盘失败丢弃该条，绝不反压
            }
        }
    }
}
