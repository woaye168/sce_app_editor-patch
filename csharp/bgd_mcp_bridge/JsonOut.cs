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
