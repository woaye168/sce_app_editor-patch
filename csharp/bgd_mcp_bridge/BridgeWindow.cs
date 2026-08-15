using System.Diagnostics;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SceEditor = SCEModule.Editor;
using SceEventManager = SCE.CppInterface.EventManager;
using SceEventObject = SCE.CppInterface.EventObject;

namespace BgdMcpBridge;

/// <summary>
/// 编辑器 C# 扩展注入入口。由 Lua 侧调用
/// SCE.Common.csharp_activate_window('BgdMcpBridge.BridgeWindow, bgd_mcp_bridge')
/// 经宿主反射创建（Type.GetType + Activator）。
/// 必须继承 Window（宿主侧有 (Window) 强转）且保留无参构造函数。
/// 宿主会无条件 Activate() 本窗口，构造后在 Activated 中立即自隐藏，作为纯后台服务窗口运行。
/// </summary>
public class BridgeWindow : Window
{
    /// <summary>单例实例，供后续 HTTP 服务模块取用。</summary>
    public static BridgeWindow? Instance { get; private set; }

    // 单例保护标记：宿主重复触发构造时只记日志不重复初始化
    private static bool _initialized;

    // 等待宿主 DI 就绪的最长时间
    private static readonly TimeSpan DiWaitTimeout = TimeSpan.FromSeconds(30);

    // DI 重试间隔
    private static readonly TimeSpan DiRetryInterval = TimeSpan.FromMilliseconds(500);

    private readonly TextBlock _statusText;
    private readonly DateTime _startTime = DateTime.Now;
    private readonly DispatcherTimer _refreshTimer;
    private McpServer? _server;

    public BridgeWindow()
    {
        if (_initialized)
        {
            Logger.Warn("BridgeWindow 重复构造，跳过初始化");
            // 重复构造时 Content 也必须赋值，否则宿主显示空窗口（虽然马上自隐藏）
            _statusText = new TextBlock();
            Content = _statusText;
            _refreshTimer = new DispatcherTimer();
            return;
        }
        _initialized = true;
        Instance = this;

        InstallGlobalExceptionGuards();

        Title = "bgd_mcp_bridge";

        _statusText = new TextBlock
        {
            Text = $"bgd_mcp_bridge 初始化中 | 启动时间: {_startTime:yyyy-MM-dd HH:mm:ss}",
            Margin = new Thickness(24)
        };
        Content = _statusText;

        // 宿主会无条件 Activate() 本窗口，Activated 后立刻隐藏（已实测可行）
        Activated += (_, _) =>
        {
            try { AppWindow.Hide(); } catch { }
        };

        // 状态页每分钟刷新一次（端口/请求计数）
        _refreshTimer = new DispatcherTimer { Interval = TimeSpan.FromMinutes(1) };
        _refreshTimer.Tick += (_, _) => RefreshStatus();
        _refreshTimer.Start();

        try
        {
            var exe = Process.GetCurrentProcess().MainModule?.FileName ?? "(unknown)";
            Logger.Info($"bgd_mcp_bridge 启动, pid={Environment.ProcessId}, exe={exe}, engineRoot={Logger.TryGetEngineRoot() ?? "(unknown)"}");
        }
        catch
        {
            // 启动日志失败不影响主流程
        }

        // 后台初始化：等宿主 DI 就绪后启动命令桥/事件缓冲/HTTP 服务
        _ = Task.Run(InitServicesAsync);
    }

    /// <summary>
    /// 等服务初始化：重试等待宿主 DI 就绪（BridgeWindow 可能早于 DI 初始化创建），
    /// 然后依次创建 EditorBridge / EventBuffer / McpServer 并启动。
    /// 全程静默，异常只记日志。
    /// </summary>
    private async Task InitServicesAsync()
    {
        try
        {
            SceEventManager? eventManager = null;
            SceEventObject? eventObject = null;
            var deadline = DateTime.UtcNow + DiWaitTimeout;
            while (DateTime.UtcNow < deadline)
            {
                try
                {
                    // Editor.Current 为空或服务未注册时 GetService 抛异常，重试等待
                    eventManager = SceEditor.GetService<SceEventManager>();
                    eventObject = SceEditor.GetService<SceEventObject>();
                    break;
                }
                catch
                {
                    await Task.Delay(DiRetryInterval).ConfigureAwait(false);
                }
            }

            if (eventManager == null || eventObject == null)
            {
                Logger.Error("宿主 DI 等待超时（30s），服务未启动");
                UpdateStatus("bgd_mcp_bridge 启动失败：宿主 DI 等待超时");
                return;
            }

            var bridge = new EditorBridge(eventManager, eventObject);
            var events = new EventBuffer();
            events.SubscribeLua(eventObject);
            // 原生事件订阅依赖 ApplicationExport 静态构造（内部走 DI），须 DI 就绪后调用
            events.SubscribeNative();

            var server = new McpServer(bridge, events);
            // ExitEditor（编辑器关闭）：进程即将退出，只做轻量清理（删端口文件），
            // 不 Stop listener——避免停机竞态弹未处理异常框（HttpListenerException 995 / ObjectDisposed）。
            events.OnExitEditor = server.OnProcessExit;

            if (!server.Start())
            {
                UpdateStatus("bgd_mcp_bridge 启动失败：端口全部被占");
                return;
            }
            _server = server;
            Logger.Info($"服务初始化完成, port={server.Port}");
            RefreshStatus();
        }
        catch (Exception ex)
        {
            Logger.Error("服务初始化失败", ex);
            UpdateStatus("bgd_mcp_bridge 启动失败，详见日志");
        }
    }

    /// <summary>
    /// 全局未处理异常守护：吞掉「停机期预期异常」（HttpListener 995 / ObjectDisposed / OperationCanceled），
    /// 防止进程关闭瞬间监听器/IO 线程的竞态异常逃逸成原生弹窗。其余异常仍记日志（不吞，便于排查）。
    /// 只过滤明确属于关闭竞态的类型，不影响其他功能。
    /// </summary>
    private static void InstallGlobalExceptionGuards()
    {
        try
        {
            TaskScheduler.UnobservedTaskException += (_, e) =>
            {
                if (IsShutdownNoise(e.Exception))
                {
                    e.SetObserved(); // 标记已观察，阻止进宿主未处理流程
                }
                else
                {
                    Logger.Error("未观察的任务异常", e.Exception);
                }
            };
            AppDomain.CurrentDomain.UnhandledException += (_, e) =>
            {
                if (e.ExceptionObject is Exception ex && !IsShutdownNoise(ex))
                {
                    Logger.Error("未处理异常", ex);
                }
                // 停机噪音：不记也不上报，进程正在退出，无需处理
            };
        }
        catch (Exception ex)
        {
            Logger.Error("安装全局异常守护失败", ex);
        }
    }

    /// <summary>判断是否「进程关闭期的预期噪音异常」（HttpListener/IO 句柄在退出时被回收所致）。</summary>
    private static bool IsShutdownNoise(Exception ex)
    {
        for (Exception? cur = ex; cur != null; cur = cur.InnerException)
        {
            if (cur is ObjectDisposedException or OperationCanceledException) return true;
            // HttpListenerException 995 = 线程退出/应用请求导致 I/O 中止（ERROR_OPERATION_ABORTED）
            if (cur is System.Net.HttpListenerException hle && hle.ErrorCode == 995) return true;
        }
        return false;
    }

    /// <summary>刷新状态页文本（端口/请求计数），切 UI 线程执行。</summary>
    private void RefreshStatus()
    {
        var server = _server;
        var text = server != null
            ? $"bgd_mcp_bridge 运行中 | 端口: {server.Port} | 请求数: {server.RequestCount} | 启动时间: {_startTime:yyyy-MM-dd HH:mm:ss}"
            : $"bgd_mcp_bridge 初始化中 | 启动时间: {_startTime:yyyy-MM-dd HH:mm:ss}";
        UpdateStatus(text);
    }

    private void UpdateStatus(string text)
    {
        try
        {
            var dq = DispatcherQueue;
            if (dq.HasThreadAccess)
            {
                _statusText.Text = text;
            }
            else
            {
                dq.TryEnqueue(() =>
                {
                    try { _statusText.Text = text; } catch { }
                });
            }
        }
        catch
        {
            // 状态更新失败不影响主流程
        }
    }
}
