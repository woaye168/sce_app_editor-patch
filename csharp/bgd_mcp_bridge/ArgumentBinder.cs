using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace BgdMcpBridge;

/// <summary>
/// 健壮参数绑定器（svc/cpp 共用，0.5.0 M3）。
/// - 重载消歧：按 JSON 参数 key 集合/个数动态匹配最佳 MethodInfo，歧义返回全部候选签名；
/// - 默认参数补齐：漏传可选参数自动填 ParameterInfo.DefaultValue；
/// - 类型宽容：枚举字符串名/数值双向；JSON number → int/long/float/double/decimal 安全转换（溢出报错不截断）；
/// - Task 解包在执行器侧（绑定器只负责入参）。
/// </summary>
public static class ArgumentBinder
{
    public sealed record BindResult(bool Ok, MethodInfo? Method, object?[]? Args, string? Error, List<string>? Candidates);

    /// <summary>在候选重载中按 args 的 key 集合/个数选最佳并绑定。</summary>
    public static BindResult BindOverloads(IReadOnlyList<MethodInfo> overloads, JsonObject? args)
    {
        if (overloads.Count == 0) return new BindResult(false, null, null, "无候选方法", null);

        // 按「必填参数全部被提供 + 提供的 key 全部合法」过滤
        var argKeys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        if (args != null)
        {
            foreach (var kv in args) argKeys.Add(kv.Key);
        }

        var scored = new List<(MethodInfo M, int Score)>();
        foreach (var m in overloads)
        {
            var ps = m.GetParameters();
            var names = new HashSet<string>(ps.Select(p => p.Name ?? ""), StringComparer.OrdinalIgnoreCase);
            // 传了方法没有的 key → 排除
            if (argKeys.Any(k => !names.Contains(k))) continue;
            // 必填缺失 → 排除
            if (ps.Any(p => !IsOptional(p) && !argKeys.Contains(p.Name ?? ""))) continue;
            // 打分：提供的 key 越多越好（消歧），参数总数越少越好（最贴合）
            scored.Add((m, argKeys.Count(k => names.Contains(k)) * 100 - ps.Length));
        }

        if (scored.Count == 0)
        {
            return new BindResult(false, null, null, "没有匹配的重载：参数名/必填项不符", CandidatesOf(overloads));
        }
        var best = scored.OrderByDescending(s => s.Score).ToList();
        if (best.Count > 1 && best[0].Score == best[1].Score)
        {
            return new BindResult(false, null, null, "重载匹配歧义，请补充参数或改用带签名后缀的能力 id", CandidatesOf(overloads));
        }
        return BindSingle(best[0].M, args);
    }

    /// <summary>绑定单一方法。</summary>
    public static BindResult BindSingle(MethodInfo method, JsonObject? args)
    {
        var ps = method.GetParameters();
        var values = new object?[ps.Length];
        for (int i = 0; i < ps.Length; i++)
        {
            var p = ps[i];
            var node = FindArg(args, p.Name ?? "");
            if (node == null)
            {
                if (IsOptional(p))
                {
                    values[i] = p.DefaultValue;
                    continue;
                }
                return new BindResult(false, method, null, $"Missing required argument: {p.Name}", null);
            }
            try
            {
                values[i] = Convert(node, p.ParameterType);
            }
            catch (Exception ex)
            {
                return new BindResult(false, method, null, $"参数 {p.Name} 转换失败（期望 {TypeName(p.ParameterType)}）: {ex.Message}", null);
            }
        }
        return new BindResult(true, method, values, null, null);
    }

    private static JsonNode? FindArg(JsonObject? args, string name)
    {
        if (args == null) return null;
        foreach (var kv in args)
        {
            if (string.Equals(kv.Key, name, StringComparison.OrdinalIgnoreCase)) return kv.Value;
        }
        return null;
    }

    private static bool IsOptional(ParameterInfo p) => p.IsOptional || p.HasDefaultValue;

    private static List<string> CandidatesOf(IReadOnlyList<MethodInfo> methods)
    {
        return methods.Select(m =>
        {
            var ps = string.Join(", ", m.GetParameters().Select(p =>
                $"{p.Name}: {TypeName(p.ParameterType)}{(IsOptional(p) ? " = " + (p.DefaultValue?.ToString() ?? "null") : "")}"));
            return $"{m.Name}({ps}) → {TypeName(m.ReturnType)}";
        }).ToList();
    }

    /// <summary>类型名简写（string/int/List&lt;string&gt; 形态，供签名展示）。</summary>
    public static string TypeName(Type t)
    {
        if (t == typeof(string)) return "string";
        if (t == typeof(bool)) return "bool";
        if (t == typeof(int)) return "int";
        if (t == typeof(uint)) return "uint";
        if (t == typeof(long)) return "long";
        if (t == typeof(float)) return "float";
        if (t == typeof(double)) return "double";
        if (t == typeof(decimal)) return "decimal";
        if (t == typeof(void)) return "void";
        if (t == typeof(object)) return "object";
        if (t.IsEnum) return "enum:" + t.Name;
        if (t.IsGenericType && t.GetGenericTypeDefinition() == typeof(List<>))
        {
            return $"List<{TypeName(t.GetGenericArguments()[0])}>";
        }
        if (t.IsArray) return TypeName(t.GetElementType()!) + "[]";
        if (t.IsGenericType && t.GetGenericTypeDefinition() == typeof(Nullable<>))
        {
            return TypeName(t.GetGenericArguments()[0]) + "?";
        }
        return t.Name;
    }

    /// <summary>JSON 节点 → 目标类型的宽容转换。</summary>
    public static object? Convert(JsonNode? node, Type target)
    {
        if (node == null)
        {
            if (target.IsValueType && Nullable.GetUnderlyingType(target) == null && !target.IsEnum)
            {
                throw new InvalidCastException("null 不能赋给值类型");
            }
            return null;
        }

        var underlying = Nullable.GetUnderlyingType(target);
        if (underlying != null) return Convert(node, underlying);

        if (target == typeof(object))
        {
            return node.GetValueKind() switch
            {
                JsonValueKind.String => node.GetValue<string>(),
                JsonValueKind.Number => TryGet<long>(node, out var l) ? l : node.GetValue<double>(),
                JsonValueKind.True => true,
                JsonValueKind.False => false,
                JsonValueKind.Null => null,
                _ => node, // array/object 保留 JsonNode（datacore 通道自行递归处理）
            };
        }
        if (target == typeof(string))
        {
            return node.GetValueKind() == JsonValueKind.String ? node.GetValue<string>() : node.ToJsonString(JsonOut.Options);
        }
        if (target == typeof(bool))
        {
            return node.GetValueKind() switch
            {
                JsonValueKind.True => true,
                JsonValueKind.False => false,
                JsonValueKind.String => node.GetValue<string>() is "true" or "1" or "yes",
                JsonValueKind.Number => node.GetValue<double>() != 0,
                _ => throw new InvalidCastException("无法转 bool"),
            };
        }
        if (target.IsEnum)
        {
            if (node.GetValueKind() == JsonValueKind.String)
            {
                return Enum.Parse(target, node.GetValue<string>(), ignoreCase: true);
            }
            var num = System.Convert.ToInt64(ToDouble(node));
            return Enum.ToObject(target, num);
        }
        if (target == typeof(int)) return checked((int)ToInt64(node));
        if (target == typeof(uint)) return checked((uint)ToInt64(node));
        if (target == typeof(long)) return ToInt64(node);
        if (target == typeof(float)) return (float)ToDouble(node);
        if (target == typeof(double)) return ToDouble(node);
        if (target == typeof(decimal)) return checked((decimal)ToDouble(node));
        if (target == typeof(byte)) return checked((byte)ToInt64(node));
        if (target == typeof(short)) return checked((short)ToInt64(node));

        if (target.IsGenericType && target.GetGenericTypeDefinition() == typeof(List<>))
        {
            var elem = target.GetGenericArguments()[0];
            if (node is not JsonArray arr) throw new InvalidCastException("期望 JSON 数组");
            var list = (System.Collections.IList)Activator.CreateInstance(target)!;
            foreach (var item in arr) list.Add(Convert(item, elem));
            return list;
        }
        if (target.IsArray)
        {
            var elem = target.GetElementType()!;
            if (node is not JsonArray arr) throw new InvalidCastException("期望 JSON 数组");
            var array = Array.CreateInstance(elem, arr.Count);
            for (int i = 0; i < arr.Count; i++) array.SetValue(Convert(arr[i], elem), i);
            return array;
        }
        throw new InvalidCastException($"不支持的目标类型 {TypeName(target)}");
    }

    private static long ToInt64(JsonNode node)
    {
        if (node.GetValueKind() == JsonValueKind.Number)
        {
            // 溢出报错而非截断
            if (TryGet<long>(node, out var l)) return l;
            var d = node.GetValue<double>();
            return checked((long)d);
        }
        if (node.GetValueKind() == JsonValueKind.String) return long.Parse(node.GetValue<string>());
        throw new InvalidCastException("无法转整数");
    }

    /// <summary>JsonValue.TryGetValue 的 JsonNode 便捷封装。</summary>
    public static bool TryGet<T>(JsonNode node, out T value)
    {
        if (node is JsonValue jv && jv.TryGetValue(out value!)) return true;
        value = default!;
        return false;
    }

    private static double ToDouble(JsonNode node)
    {
        if (node.GetValueKind() == JsonValueKind.Number) return node.GetValue<double>();
        if (node.GetValueKind() == JsonValueKind.String) return double.Parse(node.GetValue<string>());
        throw new InvalidCastException("无法转数值");
    }
}

/// <summary>返回值安全投影（0.5.0）：复杂对象浅层投影，最大深度 2（投影器自己递归计数，与全局序列化器无关）。</summary>
public static class ReturnProjector
{
    private const int MaxDepth = 2;

    /// <summary>任意返回值 → JsonNode。失败退 ToString()。</summary>
    public static JsonNode? Project(object? value)
    {
        try
        {
            return ProjectCore(value, 0);
        }
        catch
        {
            try { return JsonValue.Create(value?.ToString()); } catch { return null; }
        }
    }

    private static JsonNode? ProjectCore(object? value, int depth)
    {
        if (value == null) return null;
        var t = value.GetType();

        if (value is JsonNode node) return node.DeepClone();
        if (value is string s) return JsonValue.Create(s);
        if (value is bool b) return JsonValue.Create(b);
        if (value is char ch) return JsonValue.Create(ch.ToString());
        if (t.IsEnum) return JsonValue.Create(value.ToString());
        if (t.IsPrimitive) return JsonValue.Create(value); // int/long/float/double/char 等
        if (value is decimal or DateTime or Guid or TimeSpan) return JsonValue.Create(value.ToString());
        if (value is IntPtr or UIntPtr or Stream) return JsonValue.Create($"<{t.Name}>");
        if (value is Type) return JsonValue.Create(((Type)value).FullName);

        if (depth >= MaxDepth)
        {
            return JsonValue.Create(value.ToString());
        }

        if (value is System.Collections.IDictionary dict)
        {
            var obj = new JsonObject();
            foreach (System.Collections.DictionaryEntry kv in dict)
            {
                obj[kv.Key?.ToString() ?? ""] = ProjectCore(kv.Value, depth + 1);
            }
            return obj;
        }
        if (value is System.Collections.IEnumerable enumerable && value is not string)
        {
            var arr = new JsonArray();
            int count = 0;
            foreach (var item in enumerable)
            {
                if (++count > 200) { arr.Add("...(截断)"); break; }
                arr.Add(ProjectCore(item, depth + 1));
            }
            return arr;
        }

        // 复杂对象浅层投影：跳过索引器、跳过求值抛异常的属性（getter 可能触 native/RPC 副作用）、
        // 跳过 Stream/IntPtr 等不可序列化类型
        var result = new JsonObject { ["__type"] = t.Name };
        foreach (var prop in t.GetProperties(BindingFlags.Public | BindingFlags.Instance))
        {
            try
            {
                if (prop.GetIndexParameters().Length > 0) continue;
                if (!prop.CanRead) continue;
                var pt = prop.PropertyType;
                if (typeof(Stream).IsAssignableFrom(pt) || pt == typeof(IntPtr) || pt == typeof(UIntPtr) || pt.IsPointer) continue;
                var v = prop.GetValue(value);
                result[prop.Name] = ProjectCore(v, depth + 1);
            }
            catch
            {
                // 单个属性求值失败跳过（但注意：native AV 级别的崩溃这里防不住，svc 准入扫描就是为此存在）
            }
        }
        return result;
    }
}
