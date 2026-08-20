//! stdio MCP 聚合服务（0.5.4 自 bgd_sce_tools 迁入：应用自持，与宿主解耦）。
//!
//! 运行：`sce_app_editor-patch mcp`（由 MCP 客户端按需拉起，NDJSON 行协议，stdout 只写协议帧）。
//!
//! 工具集（恒定 8 个）：
//! - 本地实现（编辑器外）：editor_start / editor_stop / get_logs / capture_game
//! - 在线透传 bgd_mcp_bridge：start_debug（默认 restart_last_debug）/ stop_debug /
//!   publish_project / get_status；编辑器不在线时返回明确错误引导 editor_start
//!
//! project_path 缺省规则：参数 > 应用记住的最近项目（config.json last_project_path，GUI 选过项目即写入）
//! > 在线编辑器当前地图（get_status.map_path）。

use crate::core::{bridge_client, capture, editor, locate};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const PROTOCOL_VERSION: &str = "2025-03-26";

/// 解析项目路径：参数 > 应用记住的最近项目 > 在线编辑器当前地图
fn resolve_project(args: Option<&Value>) -> Result<PathBuf> {
    if let Some(p) = args
        .and_then(|a| a.get("project_path"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        return Ok(PathBuf::from(p));
    }
    if let Some(p) = editor::last_project_path() {
        return Ok(p);
    }
    Err(anyhow!(
        "未提供 project_path，且无可推导的当前项目（请显式传 project_path，或先在「编辑器补丁」应用里选择过项目）"
    ))
}

/// 目标项目在线端口（不在线给引导性错误）
fn require_online(project: &Path) -> Result<u16> {
    let target = locate::locate(project).map_err(|e| anyhow!(e))?;
    bridge_client::online_port(&target.engine_root().map_err(|e| anyhow!(e))?)
        .ok_or_else(|| anyhow!("编辑器不在线（MCP 桥不可达）。请先 editor_start 启动编辑器"))
}

// ---------------------------------------------------------------- 工具实现

fn tool_editor_start(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let wait = args.get("wait_online").and_then(|v| v.as_bool()).unwrap_or(true);
    let timeout = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(120_000);
    editor::editor_start(&project, wait, timeout).map_err(|e| anyhow!(e))
}

fn tool_editor_stop(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    editor::editor_stop(&project).map_err(|e| anyhow!(e))
}

fn tool_get_game_logs(args: &Value) -> Result<Value> {
    // 0.5.6：参数仅 source/tail_lines；项目走自动解析链（最近项目，失败明确报错）
    let project = resolve_project(None)?;
    let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let tail = args
        .get("tail_lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    editor::get_game_logs(&project, source, tail).map_err(|e| anyhow!(e))
}

fn tool_start_debug(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let port = require_online(&project)?;
    let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false);
    // 桥内 start/restart 自身超时 120s，客户端放宽到 150s
    const DEBUG_TIMEOUT_MS: u64 = 150_000;
    if full {
        return bridge_client::bridge_rpc(port, "start_debug", json!({}), DEBUG_TIMEOUT_MS);
    }
    // 默认 restart_last_debug（跳过编辑器编译构建、载入最新 lua）；无上一次调试版本时回退全量
    match bridge_client::bridge_rpc(port, "restart_last_debug", json!({}), DEBUG_TIMEOUT_MS) {
        Ok(v) => Ok(v),
        Err(e) => {
            let fallback = bridge_client::bridge_rpc(port, "start_debug", json!({}), DEBUG_TIMEOUT_MS)?;
            Ok(json!({
                "note": format!("restart_last_debug 失败（{e}），已回退全量 start_debug"),
                "result": fallback,
            }))
        }
    }
}

fn tool_stop_debug(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let port = require_online(&project)?;
    bridge_client::bridge_rpc(port, "stop_debug", json!({}), 30_000)
}

fn tool_get_status(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let port = require_online(&project)?;
    bridge_client::bridge_rpc(port, "get_status", json!({}), 15_000)
}

fn tool_publish_project(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let port = require_online(&project)?;
    let timeout = args
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(600_000);
    bridge_client::bridge_invoke(port, "lua.publish_project", json!({}), timeout)
}

fn tool_capture_game(args: &Value) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let ratio = args.get("ratio").and_then(|v| v.as_f64()).unwrap_or(1.0);
    capture::capture_game(&project, ratio, None)
}

// ---------------------------------------------------------------- MCP 协议

fn tools_list() -> Value {
    json!({
        "tools": [
            {"name":"editor_start","description":"启动星火编辑器（按项目组装启动命令，等待 MCP 桥上线；幂等：已在线直接返回）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string","description":"项目根，缺省取应用最近项目/在线编辑器当前地图"},"wait_online":{"type":"boolean","default":true},"timeout_ms":{"type":"integer","default":120000}}}},
            {"name":"editor_stop","description":"关闭星火编辑器（直接结束进程，不做优雅退出）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"start_debug","description":"启动调试（默认 restart_last_debug：跳过编辑器编译构建、载入最新 lua；无上一次调试版本自动回退全量；full=true 强制全量）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"full":{"type":"boolean","default":false}}}},
            {"name":"stop_debug","description":"停止调试","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"get_game_logs","description":"获取游戏日志/编辑器日志/MCP桥日志的最新文件信息（路径/大小/创建时间/修改时间/行数/说明）。tail_lines=0（默认）只返回文件信息不返回内容防爆上下文，需要内容时传 tail_lines 或按路径自行读取。离线可用（不要求编辑器在线）。source 取值：game_client/game_server/service_core/xdeditor_client/bridge_main/bridge_audit，或聚合前缀（如 game 命中 game_client+game_server、bridge 命中 bridge_main+bridge_audit），或 all；缺省 game","inputSchema":{"type":"object","properties":{"source":{"type":"string","default":"game","description":"日志源 key 或聚合前缀，缺省 game"},"tail_lines":{"type":"integer","default":0,"description":"返回末尾行数，0=只返回文件信息"}}}},
            {"name":"publish_project","description":"发布项目到创作者中心（分钟级耗时；需在桥 config.json danger_allow 放行 lua.publish_project）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"timeout_ms":{"type":"integer","default":600000}}}},
            {"name":"capture_game","description":"截取调试中的游戏画面/游戏截图（纯游戏画面+游戏 UI，不含编辑器界面；编辑器被遮挡/最小化均可后台截取），返回 png 路径，用 Read 查看。ratio 输出倍率（0.5/1/2/3/4）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"ratio":{"type":"number","default":1}}}},
            {"name":"get_status","description":"获取编辑器状态（地图路径/调试中/弹窗抑制）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}}
        ]
    })
}

fn call_tool(name: &str, args: &Value) -> Result<Value> {
    match name {
        "editor_start" => tool_editor_start(args),
        "editor_stop" => tool_editor_stop(args),
        "start_debug" => tool_start_debug(args),
        "stop_debug" => tool_stop_debug(args),
        "get_game_logs" => tool_get_game_logs(args),
        "publish_project" => tool_publish_project(args),
        "capture_game" => tool_capture_game(args),
        "get_status" => tool_get_status(args),
        _ => Err(anyhow!("unknown tool: {name}")),
    }
}

fn result_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn result_err(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_result(id: &Value, r: Result<Value>) -> Value {
    match r {
        Ok(v) => result_ok(
            id,
            json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }]
            }),
        ),
        Err(e) => result_ok(
            id,
            json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({"ok":false,"error":format!("{e:#}")})).unwrap_or_default() }],
                "isError": true
            }),
        ),
    }
}

fn handle(msg: &Value) -> Option<Value> {
    let method = msg.get("method")?.as_str()?;
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => Some(result_ok(
            &id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "sce_app_editor-patch", "version": env!("CARGO_PKG_VERSION") }
            }),
        )),
        "ping" => Some(result_ok(&id, json!({}))),
        "tools/list" => Some(result_ok(&id, tools_list())),
        "tools/call" => {
            let params = msg.get("params");
            let name = params.and_then(|p| p.get("name")).and_then(|v| v.as_str());
            let Some(name) = name else {
                return Some(result_err(&id, -32602, "missing tool name"));
            };
            let args = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));
            Some(tool_result(&id, call_tool(name, &args)))
        }
        m if m.starts_with("notifications/") => None,
        _ => {
            if msg.get("id").is_some() {
                Some(result_err(&id, -32601, "Method not found"))
            } else {
                None
            }
        }
    }
}

/// stdio MCP 主循环（NDJSON：每行一个 JSON-RPC 消息，stdout 只写响应帧）
pub fn run_stdio() -> i32 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&msg) {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default());
            let _ = out.flush();
        }
    }
    0
}
