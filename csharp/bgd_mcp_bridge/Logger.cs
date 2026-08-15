using System.Diagnostics;

namespace BgdMcpBridge;

/// <summary>
/// 进程内静态日志器。日志写入 <引擎运行根>/logs/bgd_csharp/bgd_csharp-YYYY-MM-DD-HH.log，
/// 按日期+小时滚动。任何一步失败均静默降级（日志直接丢弃，绝不抛异常），
/// 保证注入宿主进程时绝不因日志问题影响编辑器。
/// </summary>
public static class Logger
{
    private static readonly object _lock = new();

    // 当前日志文件路径（按小时切换），null 表示降级（日志丢弃）
    private static string? _currentFile;
    private static DateTime _currentHour;

    /// <summary>
    /// 推导引擎运行根：主 exe 位于 <运行根>/version-XX/ 下，取其目录的父目录即运行根。
    /// 任何一步失败返回 null（静默降级）。
    /// </summary>
    public static string? TryGetEngineRoot()
    {
        try
        {
            var exe = Process.GetCurrentProcess().MainModule?.FileName;
            if (string.IsNullOrEmpty(exe)) return null;
            var versionDir = Path.GetDirectoryName(exe);
            if (string.IsNullOrEmpty(versionDir)) return null;
            var root = Path.GetDirectoryName(versionDir);
            return string.IsNullOrEmpty(root) ? null : root;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>
    /// 取当前小时对应的日志文件路径；失败返回 null。
    /// 每小时首次写入自动切新文件，同时保证日志目录存在。
    /// </summary>
    private static string? GetLogFile(DateTime now)
    {
        try
        {
            var hour = new DateTime(now.Year, now.Month, now.Day, now.Hour, 0, 0);
            if (_currentFile != null && _currentHour == hour)
                return _currentFile;

            var root = TryGetEngineRoot();
            if (root == null) return null;
            var dir = Path.Combine(root, "logs", "bgd_csharp");
            Directory.CreateDirectory(dir);
            _currentFile = Path.Combine(dir, $"bgd_csharp-{now:yyyy-MM-dd-HH}.log");
            _currentHour = hour;
            return _currentFile;
        }
        catch
        {
            return null;
        }
    }

    private static void Write(string level, string message, Exception? ex = null)
    {
        try
        {
            var now = DateTime.Now;
            var file = GetLogFile(now);
            if (file == null) return; // 静默降级

            var line = $"[{now:HH:mm:ss.fff}][{level}][{Environment.CurrentManagedThreadId}] {message}";
            if (ex != null) line += Environment.NewLine + ex;
            line += Environment.NewLine;

            lock (_lock)
            {
                // 二次取文件：拿到锁后可能已跨小时
                var file2 = GetLogFile(DateTime.Now);
                if (file2 == null) return;
                File.AppendAllText(file2, line);
            }
        }
        catch
        {
            // 日志失败不影响主流程
        }
    }

    public static void Info(string message) => Write("INFO", message);

    public static void Warn(string message) => Write("WARN", message);

    public static void Error(string message, Exception? ex = null) => Write("ERROR", message, ex);
}
