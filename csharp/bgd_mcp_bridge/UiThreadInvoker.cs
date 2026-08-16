namespace BgdMcpBridge;

/// <summary>
/// UI 线程执行器（0.5.0 执行器共性约束）。
/// 引擎对象一律经 BridgeWindow.DispatcherQueue.TryEnqueue 调度到 UI 线程执行，
/// 用 TaskCompletionSource 取回结果并设硬超时；执行路径上绝不出现 .Wait()/.Result（防 UI 死锁）。
/// 超时视为「疑似模态阻塞」（弹窗卡住 UI 线程），记日志防后续任务堆积雪崩。
/// </summary>
public static class UiThreadInvoker
{
    /// <summary>统一硬超时默认值（invoke_capability 可用 timeout_ms 按调用覆盖）。</summary>
    public const int DefaultTimeoutMs = 5000;

    /// <summary>UI 线程调用结果。</summary>
    /// <param name="Ok">是否在超时前正常返回（异常也算返回，见 Error）。</param>
    /// <param name="Value">返回值（func 抛异常时为 null）。</param>
    /// <param name="Error">func 抛出的异常（已解包 TargetInvocationException 反射壳）。</param>
    /// <param name="TimedOut">是否超时（疑似模态阻塞）。</param>
    public sealed record UiResult(bool Ok, object? Value, Exception? Error, bool TimedOut);

    /// <summary>
    /// 在 UI 线程执行 func 并取回结果。timeoutMs ≤ 0 时用默认硬超时。
    /// 绝不抛异常；超时/投递失败/执行异常都体现在返回值里。
    /// </summary>
    public static async Task<UiResult> InvokeAsync(Func<object?> func, int timeoutMs = DefaultTimeoutMs)
    {
        if (timeoutMs <= 0) timeoutMs = DefaultTimeoutMs;
        try
        {
            var dq = BridgeWindow.Instance?.DispatcherQueue;
            if (dq == null)
            {
                return new UiResult(false, null, new InvalidOperationException("UI 线程不可用（BridgeWindow 未初始化）"), false);
            }
            if (dq.HasThreadAccess)
            {
                try
                {
                    return new UiResult(true, func(), null, false);
                }
                catch (Exception ex)
                {
                    return new UiResult(true, null, Unwrap(ex), false);
                }
            }

            var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
            if (!dq.TryEnqueue(() =>
                {
                    try { tcs.TrySetResult(func()); }
                    catch (Exception ex) { tcs.TrySetException(ex); }
                }))
            {
                return new UiResult(false, null, new InvalidOperationException("UI 线程投递失败（TryEnqueue 返回 false）"), false);
            }

            var completed = await Task.WhenAny(tcs.Task, Task.Delay(timeoutMs)).ConfigureAwait(false);
            if (completed != tcs.Task)
            {
                Logger.Warn($"UI 线程调用超时({timeoutMs}ms)，疑似模态阻塞（弹窗未抑制？）");
                return new UiResult(false, null, null, true);
            }
            try
            {
                return new UiResult(true, await tcs.Task.ConfigureAwait(false), null, false);
            }
            catch (Exception ex)
            {
                return new UiResult(true, null, Unwrap(ex), false);
            }
        }
        catch (Exception ex)
        {
            Logger.Error("UI 线程调用异常", ex);
            return new UiResult(false, null, ex, false);
        }
    }

    /// <summary>递归解包反射壳：MethodInfo.Invoke 的业务异常必裹在 TargetInvocationException 里，绝不把壳透给 AI。</summary>
    public static Exception Unwrap(Exception ex)
    {
        while (ex is System.Reflection.TargetInvocationException && ex.InnerException != null)
        {
            ex = ex.InnerException;
        }
        return ex;
    }
}
