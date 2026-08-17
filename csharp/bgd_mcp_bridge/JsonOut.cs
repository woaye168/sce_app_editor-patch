using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;

namespace BgdMcpBridge;

/// <summary>
/// 全局统一 JSON 出口（0.5.0 M1）。
/// 所有对外的 JSON 序列化必须走这里，保证：
/// 1. 中文不转义（UnsafeRelaxedJsonEscaping，消灭 \uXXXX）；
/// 2. 引擎对象循环引用不炸（IgnoreCycles）。
/// 注意：刻意<b>不</b>在全局单例上设 MaxDepth——MaxDepth 读写双向生效，会把正常
/// MCP/rpc 请求体（嵌套 params）和错误自愈响应炸掉；「最大深度 2」是返回值浅层
/// 投影器（<see cref="ReturnProjector"/>）自己的实现细节，与本全局序列化器无关。
/// </summary>
public static class JsonOut
{
    /// <summary>全局唯一序列化选项单例。</summary>
    public static readonly JsonSerializerOptions Options = CreateOptions();

    private static JsonSerializerOptions CreateOptions()
    {
        return new JsonSerializerOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            ReferenceHandler = ReferenceHandler.IgnoreCycles,
        };
    }

    /// <summary>JsonNode → 字符串（不转义中文）。</summary>
    public static string Stringify(JsonNode? node)
    {
        return node?.ToJsonString(Options) ?? "null";
    }

    /// <summary>JsonNode → 缩进字符串（tools/call content 等人读场景）。</summary>
    public static string StringifyPretty(JsonNode? node)
    {
        if (node == null) return "null";
        var opts = new JsonSerializerOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            ReferenceHandler = ReferenceHandler.IgnoreCycles,
            WriteIndented = true,
        };
        return node.ToJsonString(opts);
    }

    /// <summary>任意对象 → 字符串（不转义中文）。</summary>
    public static string Serialize<T>(T value)
    {
        return JsonSerializer.Serialize(value, Options);
    }
}

/// <summary>
/// JSON 入参的<b>零异常</b>宽容读取（0.5.0 自测自修轮新增）。
/// 背景：编辑器对 first-chance 异常弹「发生异常」模态（即使异常被 catch 吞掉），
/// 因此「GetValue&lt;int&gt;() 抛了再 catch」式防御解析在本进程内等同于弹窗——必须靠
/// ValueKind 预判 + TryGetValue 做到正常路径完全不抛异常。类型不符时记 WARN 日志
/// （保证可观测性：之前静默 catch 导致错误无任何日志可查）。
/// </summary>
public static class JsonRead
{
    /// <summary>读 int：Number 直读；数字字符串宽容解析；其他类型记日志返回默认值。</summary>
    public static int Int(JsonObject? obj, string key, int defaultValue)
    {
        return (int)Long(obj, key, defaultValue);
    }

    /// <summary>读 long：Number 直读；数字字符串宽容解析；其他类型记日志返回默认值。</summary>
    public static long Long(JsonObject? obj, string key, long defaultValue)
    {
        var node = obj?[key];
        if (node == null) return defaultValue;
        var kind = node.GetValueKind();
        if (kind == JsonValueKind.Number)
        {
            if (node is JsonValue jv && jv.TryGetValue(out long l)) return l;
            if (node is JsonValue jv2 && jv2.TryGetValue(out double d)) return (long)d;
        }
        if (kind == JsonValueKind.String)
        {
            var s = (node as JsonValue)?.TryGetValue(out string? sv) == true ? sv : null;
            if (s != null && long.TryParse(s, out var l2)) return l2;
        }
        Logger.Warn($"参数 {key} 类型不符（{kind}），用默认值 {defaultValue}");
        return defaultValue;
    }

    /// <summary>读 string：String 直读；其他标量转文本；缺失/异常返回 null（调用方判空）。</summary>
    public static string? Str(JsonObject? obj, string key)
    {
        var node = obj?[key];
        if (node == null) return null;
        var kind = node.GetValueKind();
        if (kind == JsonValueKind.String)
        {
            return (node as JsonValue)?.TryGetValue(out string? sv) == true ? sv : null;
        }
        if (kind is JsonValueKind.Number or JsonValueKind.True or JsonValueKind.False)
        {
            Logger.Warn($"参数 {key} 类型不符（{kind}），按字符串处理");
            return node.ToString();
        }
        return null;
    }

    /// <summary>读 bool：True/False 直读；字符串 "true"/"1" 宽容；其他记日志返回默认值。</summary>
    public static bool Bool(JsonObject? obj, string key, bool defaultValue)
    {
        var node = obj?[key];
        if (node == null) return defaultValue;
        var kind = node.GetValueKind();
        if (kind == JsonValueKind.True) return true;
        if (kind == JsonValueKind.False) return false;
        if (kind == JsonValueKind.String)
        {
            var s = (node as JsonValue)?.TryGetValue(out string? sv) == true ? sv : null;
            if (s is "true" or "1" or "yes") return true;
            if (s is "false" or "0" or "no") return false;
        }
        Logger.Warn($"参数 {key} 类型不符（{kind}），用默认值 {defaultValue}");
        return defaultValue;
    }
}
