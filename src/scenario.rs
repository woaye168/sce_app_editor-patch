//! 场景脚本执行器（0.8.0）：AI 写完功能后把「起调试 → 操作 UI → 验证」编成步骤数组一次跑完，
//! 替代十几次单步 MCP 往返。
//!
//! 步骤 op：
//!   invoke      { id: <能力id>, args?, timeout_ms? }  调桥能力（lua.* 等）
//!   start_debug / stop_debug {}                       调试启停（默认 restart_last_debug）
//!   capture     { ratio?, crop?, max_width? }         截图（返回 png 路径）
//!   logs        { source?, tail_lines?, match? }      读日志（含 errors 上浮）
//!   wait        { ms }                                等待（界面动画/协议往返）
//!   note        { text }                              标记注释（对齐结果与步骤）
//!
//! 默认遇错即停（stop_on_error=false 继续）。每步结果截断 2KB，整体还有 32KB 护栏。

use crate::core::{bridge_client, capture, logs};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

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

pub fn run_scenario(args: &Value) -> Result<Value> {
    let project = crate::mcp::resolve_project_pub(Some(args))?;
    let steps = args
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("steps 缺失（步骤数组：invoke/capture/logs/wait/note/start_debug/stop_debug）"))?;
    let stop_on_error = args
        .get("stop_on_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let t0 = std::time::Instant::now();
    let mut results: Vec<Value> = Vec::new();
    let mut failed_step = Value::Null;

    for (i, step) in steps.iter().enumerate() {
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
                let max_width = step
                    .get("max_width")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32)
                    .or(Some(1280));
                capture::capture_game_mcp(&project, ratio, crop, max_width)
            }
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
            _ => Err(anyhow!("未知 op: '{op}'（可用：invoke/start_debug/stop_debug/capture/logs/wait/note）")),
        };
        match r {
            Ok(v) => results.push(json!({ "step": i, "op": op, "ok": true, "result": step_cap(v) })),
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
        "hint": "步骤全部成功=绿；failed_step 非空先看该步 error 与最近 logs(errors 段)。截图步骤返回 path 用 Read 查看",
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
}
