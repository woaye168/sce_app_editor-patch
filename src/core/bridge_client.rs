//! bgd_mcp_bridge 进程内桥的 HTTP 客户端（0.5.4 自 bgd_sce_tools 迁入）。
//!
//! 在线判定：`<运行根>/logs/bgd_csharp/port` 端口文件 + `/mcp` initialize 握手双重确认
//!（强杀编辑器后 port 文件可能残留，握手失败即视为离线）。

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// 读端口文件（不存在/非法返回 None）
pub fn read_port(engine_root: &Path) -> Option<u16> {
    let path = engine_root.join("logs").join("bgd_csharp").join("port");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
}

/// 探测桥是否在线（POST /mcp initialize 握手，短超时）
pub fn bridge_online(port: u16) -> bool {
    bridge_rpc(port, "initialize", json!({}), 5_000).is_ok()
}

/// 调桥（HTTP JSON-RPC；initialize 走 /mcp，其余走 /rpc）。
/// timeout_ms 为客户端总超时：长操作（start_debug 桥内 120s 轮询）必须放大。
pub fn bridge_rpc(port: u16, method: &str, params: Value, timeout_ms: u64) -> Result<Value> {
    let (path, body) = if method == "initialize" {
        (
            "/mcp",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":params}),
        )
    } else {
        ("/rpc", json!({"id":1,"method":method,"params":params}))
    };
    post(port, path, &body, timeout_ms)
}

/// 调桥能力目录（invoke_capability 封装），timeout_ms 透传
pub fn bridge_invoke(port: u16, id: &str, args: Value, timeout_ms: u64) -> Result<Value> {
    let body = json!({
        "id": 1,
        "method": "invoke_capability",
        "params": { "id": id, "args": args, "timeout_ms": timeout_ms }
    });
    post(port, "/rpc", &body, timeout_ms + 5_000)
}

fn post(port: u16, path: &str, body: &Value, timeout_ms: u64) -> Result<Value> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()?;
    let resp = client
        .post(format!("http://127.0.0.1:{port}{path}"))
        .json(body)
        .send()
        .with_context(|| format!("连接编辑器桥失败（127.0.0.1:{port}）"))?;
    let text = resp.text()?;
    let doc: Value = serde_json::from_str(&text).context("桥响应不是合法 JSON")?;
    if let Some(err) = doc.get("error") {
        return Err(anyhow!("桥返回错误: {err}"));
    }
    Ok(doc.get("result").cloned().unwrap_or(Value::Null))
}

/// 在线则返回端口（port 文件 + 握手双重确认）
pub fn online_port(engine_root: &Path) -> Option<u16> {
    let port = read_port(engine_root)?;
    if bridge_online(port) {
        Some(port)
    } else {
        None
    }
}
