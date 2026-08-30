//! 场景脚本执行器（0.8.0）：AI 写完功能后把「起调试 → 操作 UI → 验证」编成步骤数组一次跑完，
//! 替代十几次单步 MCP 往返。0.8.2 进化：步骤变量 / 轮询等待 / 断言。
//!
//! 步骤 op：
//!   invoke      { id: <能力id>, args?, timeout_ms? }  调桥能力（lua.* 等）
//!   start_debug / stop_debug {}                       调试启停（默认 restart_last_debug）
//!   capture     { ratio?, crop?, max_width? }         截图（返回 png 路径）
//!   capture_ui  { q, pad?, max_width? }               复合：find_ui(q) 拿 rect → 局部截图一步完成
//!   logs        { source?, tail_lines?, match? }      读日志（含 errors 上浮）
//!   wait        { ms }                                等待（界面动画/协议往返）
//!   note        { text }                              标记注释（对齐结果与步骤）
//!   wait_for    { q|id, present?, timeout_ms? }       轮询 find_ui 直到文本/控件出现或消失
//!   assert_text { q, present? }                       断言文本存在（包含匹配，失败=步骤失败）
//!
//! 步骤变量（0.8.2）：任意步骤可带 save_as: "名"——存结果标量（默认取 clickable_ancestor
//! 或 id 或 items[0] 同名字段；save_field: "字段" 指定其他标量）；后续步骤任意字符串字段
//! 写 {$名} 引用（整串恰好为占位符时按原 JSON 类型替换，否则串内插值）。变量未定义=步骤
//! 显式报错。作用域=单次 run_scenario 调用。
//!
//! 默认遇错即停（stop_on_error=false 继续）。每步结果截断 2KB，整体还有 32KB 护栏。

use crate::core::{bridge_client, capture, logs};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 单步结果体截断上限（防单步大结果挤占整体预算）
const STEP_CAP: usize = 2048;

fn step_cap(v: Value) -> Value {
    let s = serde_json::to_string(&v).unwrap_or_default();
    if s.len() <= STEP_CAP {
        return v;
    }
    // 截断必须落在字符边界（中文多字节，直接按字节切会 panic——0.8.0 验收实测
    // 「点」字横跨 2047..2050 导致 stdio 进程崩溃 CONNECTION_CLOSED）
    let mut end = STEP_CAP;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    json!(format!(
        "{}...[步骤结果已截断：{} 字节 > {} 上限，请用更精确参数]",
        &s[..end],
        s.len(),
        STEP_CAP
    ))
}

/// 步骤变量表（单次 run_scenario 作用域）
type Vars = BTreeMap<String, Value>;

/// 递归替换步骤内所有字符串字段的 {$名} 引用。
/// 整串恰为占位符 → 按变量原 JSON 类型整体替换；串内出现 → 文本插值（变量须标量）。
fn subst_vars(v: &Value, vars: &Vars) -> Result<Value> {
    Ok(match v {
        Value::String(s) => {
            // 整串占位符：保类型替换
            if let Some(name) = s.strip_prefix("{$").and_then(|x| x.strip_suffix('}')) {
                if !name.is_empty() && !name.contains('$') {
                    return vars
                        .get(name)
                        .cloned()
                        .ok_or_else(|| anyhow!("变量未定义：${}（先在前置 find/invoke 步骤 save_as）", name));
                }
            }
            // 串内插值（替换后从插入值之后继续扫——变量值本身含 {$ 模式
            // 不会被二次展开，也不会误报「变量未定义」）
            let mut out = s.clone();
            let mut pos = 0;
            while let Some(rel) = out[pos..].find("{$") {
                let start = pos + rel;
                let Some(endrel) = out[start..].find('}') else { break };
                let end = start + endrel;
                let name = &out[start + 2..end];
                let val = vars
                    .get(name)
                    .ok_or_else(|| anyhow!("变量未定义：${}（先在前置步骤 save_as）", name))?;
                let text = match val {
                    Value::String(t) => t.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => return Err(anyhow!("变量 ${} 不是标量，不能串内插值", name)),
                };
                out.replace_range(start..=end, &text);
                pos = start + text.len();
            }
            Value::String(out)
        }
        Value::Array(a) => Value::Array(
            a.iter()
                .map(subst_vars_inner(vars))
                .collect::<Result<Vec<_>>>()?,
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| subst_vars(x, vars).map(|y| (k.clone(), y)))
                .collect::<Result<serde_json::Map<_, _>>>()?,
        ),
        _ => v.clone(),
    })
}

fn subst_vars_inner(vars: &Vars) -> impl Fn(&Value) -> Result<Value> + '_ {
    move |x| subst_vars(x, vars)
}

/// save_as 提取：默认 clickable_ancestor → id → items[0] 同名字段；save_field 指定其他标量。
fn extract_save(result: &Value, save_field: Option<&str>) -> Result<Value> {
    let pick = |v: &Value, field: &str| v.get(field).cloned();
    let v = match save_field {
        Some(f) => pick(result, f)
            .or_else(|| result.get("items").and_then(|a| a.get(0)).and_then(|i| pick(i, f))),
        None => pick(result, "clickable_ancestor")
            .or_else(|| pick(result, "id"))
            .or_else(|| {
                result.get("items").and_then(|a| a.get(0)).and_then(|i| {
                    pick(i, "clickable_ancestor").or_else(|| pick(i, "id"))
                })
            }),
    };
    match v {
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Bool(_))) => Ok(v),
        Some(_) => Err(anyhow!("save_as 只存标量（id/文本/数值），取到的是复合值")),
        None => Err(anyhow!(
            "save_as 无可存字段（结果无 clickable_ancestor/id/items[0]；或用 save_field 指定）"
        )),
    }
}

/// capture_ui 步骤：find_ui(q) 拿控件 rect → 按 rect+pad 局部截图。看单个 UI 用此步，
/// 免「先 find 再手抄坐标 crop」两次往返（全视口截图只在看整体布局时用）。
/// max_width 缺省 640（响应回显 downscaled/natural_*，像素级判读时显式调大）。
fn step_capture_ui(project: &std::path::Path, step: &Value) -> Result<Value> {
    let q = step
        .get("q")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("capture_ui 步骤缺 q（文本/id 子串，与 find_ui 同语义）"))?;
    let pad = step.get("pad").and_then(|v| v.as_f64()).unwrap_or(8.0);
    let max_width = crate::mcp::parse_max_width(step, 640)?;
    let port = crate::mcp::require_online_pub(project)?;
    let res = bridge_client::bridge_invoke(port, "lua.find_ui", json!({ "q": q }), 10_000)?;
    let item = res
        .get("items")
        .and_then(|a| a.get(0))
        .ok_or_else(|| anyhow!("capture_ui 未命中：'{q}'（先 find_ui 确认文本/id）"))?;
    let candidates = res.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let g = |r: &Value, k: &str| r.get(k).and_then(|v| v.as_f64());
    let rect = item
        .get("rect")
        .and_then(|r| match (g(r, "x"), g(r, "y"), g(r, "w"), g(r, "h")) {
            (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
            _ => None,
        })
        .ok_or_else(|| anyhow!("capture_ui 命中项无 rect（不可见？）：'{q}'"))?;
    let crop = Some((rect.0 - pad, rect.1 - pad, rect.2 + pad * 2.0, rect.3 + pad * 2.0));
    let mut out = capture::capture_game_mcp(project, 1.0, crop, max_width)?;
    if candidates > 1 {
        // 多命中取第一条属「静默默认」——显式回显候选数防误判（0.8.3 防呆纪律）
        out["candidates"] = json!(candidates);
        let prev = out.get("note").and_then(|v| v.as_str()).unwrap_or("");
        out["note"] = json!(format!(
            "q 多命中 {candidates} 个候选，已取第一个（不是目标请缩小 q）{prev}"
        ));
    }
    Ok(out)
}

pub fn run_scenario(args: &Value) -> Result<Value> {
    let project = crate::mcp::resolve_project_pub(Some(args))?;
    let steps = args
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("steps 缺失（步骤数组：invoke/capture/logs/wait/note/start_debug/stop_debug/wait_for/assert_text）"))?;
    let stop_on_error = args
        .get("stop_on_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let t0 = std::time::Instant::now();
    let mut results: Vec<Value> = Vec::new();
    let mut failed_step = Value::Null;
    let mut vars: Vars = Vars::new();

    for (i, step_raw) in steps.iter().enumerate() {
        // 步骤变量替换（在 op 解析前，全字段生效）
        let step = match subst_vars(step_raw, &vars) {
            Ok(s) => s,
            Err(e) => {
                failed_step = json!(i);
                results.push(json!({ "step": i, "ok": false, "error": format!("{e:#}") }));
                if stop_on_error {
                    break;
                }
                continue;
            }
        };
        let op = step.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let r: Result<Value> = match op {
            "invoke" => {
                let id = step
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("invoke 步骤缺 id"))?;
                let timeout = step
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10_000);
                let port = crate::mcp::require_online_pub(&project)?;
                bridge_client::bridge_invoke(
                    port,
                    id,
                    step.get("args").cloned().unwrap_or(json!({})),
                    timeout,
                )
            }
            "start_debug" => crate::mcp::tool_start_debug_pub(&json!({"project_path": project})),
            "stop_debug" => crate::mcp::tool_stop_debug_pub(&json!({"project_path": project})),
            "capture" => {
                let ratio = step.get("ratio").and_then(|v| v.as_f64()).unwrap_or(1.0);
                let crop = step.get("crop").and_then(|c| {
                    let g = |k: &str| c.get(k).and_then(|v| v.as_f64());
                    match (g("x"), g("y"), g("w"), g("h")) {
                        (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
                        _ => None,
                    }
                });
                let max_width = crate::mcp::parse_max_width(&step, 1280)?;
                capture::capture_game_mcp(&project, ratio, crop, max_width)
            }
            "capture_ui" => step_capture_ui(&project, &step),
            "logs" => {
                let source = step.get("source").and_then(|v| v.as_str()).unwrap_or("");
                let tail = step
                    .get("tail_lines")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let m = step.get("match").and_then(|v| v.as_str());
                logs::get_game_logs(&project, source, tail, m).map_err(|e| anyhow!(e))
            }
            "wait" => {
                let ms = step.get("ms").and_then(|v| v.as_u64()).unwrap_or(500).min(60_000);
                std::thread::sleep(std::time::Duration::from_millis(ms));
                Ok(json!({ "waited_ms": ms }))
            }
            "note" => Ok(json!({ "note": step.get("text").cloned().unwrap_or(Value::Null) })),
            "wait_for" | "assert_text" => {
                // 轮询/断言文本（find_ui 包含匹配语义；多命中取存在性）
                let q = step
                    .get("q")
                    .or_else(|| step.get("id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("{op} 步骤缺 q（文本/id 子串）"))?;
                let present = step.get("present").and_then(|v| v.as_bool()).unwrap_or(true);
                let timeout = if op == "wait_for" {
                    step.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5_000)
                } else {
                    0
                };
                let port = crate::mcp::require_online_pub(&project)?;
                let start = std::time::Instant::now();
                loop {
                    // 桥暂时不可用/单次超时（PIE 重启窗口期）视为「本轮未命中」继续
                    // 轮询到 timeout——wait_for 的核心场景正是等界面在重启后出现；
                    // assert_text 无轮询语义，桥错误直接上抛
                    match bridge_client::bridge_invoke(port, "lua.find_ui", json!({ "q": q }), 10_000)
                    {
                        Ok(res) => {
                            let total = res.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
                            let found = total > 0;
                            if found == present {
                                break Ok(json!({ "q": q, "present": present, "total": total,
                                    "elapsed_ms": start.elapsed().as_millis() }));
                            }
                            if op == "assert_text" || start.elapsed().as_millis() as u64 >= timeout
                            {
                                break Err(anyhow!(
                                    "{op} 失败：'{q}' 期望 present={} 实际 total={}（{}ms）",
                                    present, total, start.elapsed().as_millis()
                                ));
                            }
                        }
                        Err(e) => {
                            if op == "assert_text" {
                                break Err(e);
                            }
                            if start.elapsed().as_millis() as u64 >= timeout {
                                break Err(anyhow!(
                                    "wait_for 失败：'{q}' 期望 present={}，轮询期间桥调用持续失败（{}ms；最后错误：{e:#}）",
                                    present,
                                    start.elapsed().as_millis()
                                ));
                            }
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
            _ => Err(anyhow!("未知 op: '{op}'（可用：invoke/start_debug/stop_debug/capture/capture_ui/logs/wait/note/wait_for/assert_text）")),
        };
        match r {
            Ok(v) => {
                // save_as 存变量（失败=步骤失败）
                if let Some(name) = step.get("save_as").and_then(|v| v.as_str()) {
                    match extract_save(&v, step.get("save_field").and_then(|x| x.as_str())) {
                        Ok(val) => {
                            vars.insert(name.to_string(), val);
                        }
                        Err(e) => {
                            failed_step = json!(i);
                            results.push(json!({ "step": i, "op": op, "ok": false, "error": format!("save_as '{name}': {e:#}") }));
                            if stop_on_error {
                                break;
                            }
                            continue;
                        }
                    }
                }
                results.push(json!({ "step": i, "op": op, "ok": true, "result": step_cap(v) }))
            }
            Err(e) => {
                failed_step = json!(i);
                results.push(json!({ "step": i, "op": op, "ok": false, "error": format!("{e:#}") }));
                if stop_on_error {
                    break;
                }
            }
        }
    }

    Ok(json!({
        "results": results,
        "failed_step": failed_step,
        "elapsed_ms": t0.elapsed().as_millis(),
        "hint": "步骤全部成功=绿；failed_step 非空先看该步 error 与最近 logs(errors 段)。截图步骤返回 path 用 Read 查看；变量用 save_as/{$名} 串联 find→click",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 截断落在多字节字符中间时不得 panic（0.8.0 验收实机事故：「点」横跨 2047..2050）
    #[test]
    fn test_step_cap_multibyte_boundary() {
        // 构造：前置 ASCII 填满到 2047，随后跟「点」（3 字节 2047..2050）
        let mut s = "a".repeat(2047);
        s.push('点');
        s.push_str(&"b".repeat(100));
        let v = json!({ "text": s });
        let capped = step_cap(v);
        let out = capped.as_str().unwrap();
        assert!(out.contains("步骤结果已截断"));
        assert!(out.contains(&"a".repeat(100)));
    }

    #[test]
    fn test_step_cap_small_passthrough() {
        let v = json!({ "ok": true });
        assert_eq!(step_cap(v.clone()), v);
    }

    #[test]
    fn test_subst_vars_whole_and_inline() {
        let mut vars: Vars = Vars::new();
        vars.insert("btn".to_string(), json!("a/b/c"));
        vars.insert("n".to_string(), json!(3));
        // 整串占位符保类型
        let v = subst_vars(&json!({ "id": "{$btn}", "x": "{$n}" }), &vars).unwrap();
        assert_eq!(v["id"], json!("a/b/c"));
        assert_eq!(v["x"], json!(3));
        // 串内插值 + 嵌套数组
        let v = subst_vars(&json!({ "q": ["前缀{$btn}后缀"] }), &vars).unwrap();
        assert_eq!(v["q"][0], json!("前缀a/b/c后缀"));
        // 无占位符原样
        let v = subst_vars(&json!({ "q": "普通文本" }), &vars).unwrap();
        assert_eq!(v["q"], json!("普通文本"));
    }

    #[test]
    fn test_subst_vars_undefined_errors() {
        let vars: Vars = Vars::new();
        assert!(subst_vars(&json!("{$missing}"), &vars).is_err());
        assert!(subst_vars(&json!("嵌套 {$missing} 引用"), &vars).is_err());
    }

    /// 变量值本身含 {$ 模式不得二次展开/误报（替换后从插入值之后继续扫）
    #[test]
    fn test_subst_vars_value_with_placeholder_pattern() {
        let mut vars: Vars = Vars::new();
        vars.insert("txt".to_string(), json!("字面 {$not_a_var} 文本"));
        let v = subst_vars(&json!({ "q": "前缀{$txt}后缀" }), &vars).unwrap();
        assert_eq!(v["q"], json!("前缀字面 {$not_a_var} 文本后缀"));
    }

    #[test]
    fn test_parse_max_width_zero_rejected() {
        assert!(crate::mcp::parse_max_width(&json!({ "max_width": 0 }), 640).is_err());
        assert_eq!(
            crate::mcp::parse_max_width(&json!({}), 640).unwrap(),
            Some(640)
        );
        assert_eq!(
            crate::mcp::parse_max_width(&json!({ "max_width": 2000 }), 640).unwrap(),
            Some(2000)
        );
    }

    #[test]
    fn test_extract_save_defaults() {
        // find_ui 结果：默认存 items[0].clickable_ancestor
        let r = json!({ "items": [{ "id": "a/b/txt", "clickable_ancestor": "a/b" }] });
        assert_eq!(extract_save(&r, None).unwrap(), json!("a/b"));
        // 无 ancestor 退 id
        let r = json!({ "items": [{ "id": "a/b" }] });
        assert_eq!(extract_save(&r, None).unwrap(), json!("a/b"));
        // 顶层 id（click_ui 类结果没有这些字段 → 报错）
        let r = json!({ "clicked": "a/b" });
        assert!(extract_save(&r, None).is_err());
        // save_field 指定
        let r = json!({ "items": [{ "id": "a/b", "text": "商店" }] });
        assert_eq!(extract_save(&r, Some("text")).unwrap(), json!("商店"));
        // 复合值拒绝
        let r = json!({ "items": [{ "rect": { "x": 1 } }] });
        assert!(extract_save(&r, Some("rect")).is_err());
    }
}
