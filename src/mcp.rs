//! stdio MCP 聚合服务（0.5.4 自 bgd_sce_tools 迁入：应用自持，与宿主解耦）。
//!
//! 运行：`sce_app_editor-patch mcp`（由 MCP 客户端按需拉起，NDJSON 行协议，stdout 只写协议帧）。
//!
//! 工具集（恒定 15 个，静态列表）：
//! - 本地实现（编辑器外）：editor_start / editor_stop / get_game_logs / capture_game / run_scenario
//! - 在线透传 bgd_mcp_bridge：start_debug（默认 restart_last_debug）/ stop_debug /
//!   publish_project / get_status；编辑器不在线时返回明确错误引导 editor_start
//! - 在线透传桥 Gateway 元工具（0.7.2 起）：search_capabilities / describe_capability /
//!   invoke_capability / list_namespaces / get_events / set_suppress —— 编辑器完整能力
//!   （能力目录数百条）经 search → invoke 两步触达；离线同样报错引导 editor_start
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

// ---------------------------------------------------------------- 供 scenario 复用的 pub(crate) 门面

pub(crate) fn resolve_project_pub(args: Option<&Value>) -> Result<PathBuf> {
    resolve_project(args)
}
pub(crate) fn require_online_pub(project: &Path) -> Result<u16> {
    require_online(project)
}
pub(crate) fn tool_start_debug_pub(args: &Value) -> Result<Value> {
    tool_start_debug(args)
}
pub(crate) fn tool_stop_debug_pub(args: &Value) -> Result<Value> {
    tool_stop_debug(args)
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
    // 0.8.0 R1.3：新增 match 正则过滤（与 tail_lines 同时给出时 match 优先）
    let project = resolve_project(None)?;
    let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("");
    let tail = args
        .get("tail_lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;
    let match_pat = args.get("match").and_then(|v| v.as_str());
    editor::get_game_logs(&project, source, tail, match_pat).map_err(|e| anyhow!(e))
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
    // R1.2：crop = {x,y,w,h} 游戏视口逻辑坐标（原点=视口左上，与 lua.get_game_view_rect 同系）
    let crop = match args.get("crop") {
        None => None,
        Some(c) => {
            let g = |k: &str| c.get(k).and_then(|v| v.as_f64());
            match (g("x"), g("y"), g("w"), g("h")) {
                (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
                _ => {
                    return Err(anyhow!(
                        "crop 需为 {{\"x\",\"y\",\"w\",\"h\"}} 数字对象（游戏视口逻辑坐标，原点=视口左上；越界自动 clamp）"
                    ))
                }
            }
        }
    };
    // max_width 缺省 1280（上下文防爆），与 ratio 同时给出时为最终上限
    let max_width = args
        .get("max_width")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .or(Some(1280));
    capture::capture_game_mcp(&project, ratio, crop, max_width)
}

// ---------------------------------------------------------------- 桥 Gateway 元工具透传

/// 透传桥 Gateway 元工具：剥离 project_path 后原样转发 params。
/// timeout_ms 为客户端总超时（invoke_capability 按 params.timeout_ms + 缓冲计算）。
fn tool_bridge_meta(args: &Value, method: &str) -> Result<Value> {
    let project = resolve_project(Some(args))?;
    let port = require_online(&project)?;
    let mut params = args.clone();
    // R1.4：get_events 的 limit 由 MCP 层本地消费（桥侧不改），防长会话事件缓冲灌入
    let events_limit = if method == "get_events" {
        params
            .as_object_mut()
            .and_then(|o| o.remove("limit"))
            .and_then(|v| v.as_u64())
            .unwrap_or(50) as usize
    } else {
        0
    };
    if let Some(o) = params.as_object_mut() {
        o.remove("project_path");
    }
    let timeout_ms = if method == "invoke_capability" {
        // 桥内默认 5000；客户端放宽缓冲（与 bridge_invoke 一致 +5s）
        params
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(5_000)
            + 5_000
    } else {
        30_000
    };
    let result = bridge_client::bridge_rpc(port, method, params, timeout_ms)?;
    if method == "get_events" {
        return Ok(slice_events(result, events_limit));
    }
    Ok(result)
}

/// get_events 本地切片（R1.4）：只留最新 limit 条（默认 50）+ 告知总数/丢弃数。
fn slice_events(mut v: Value, limit: usize) -> Value {
    let Some(events) = v.get_mut("events").and_then(|e| e.as_array_mut()) else {
        return v;
    };
    let total = events.len();
    if total > limit {
        let drop = total - limit;
        events.drain(..drop);
        if let Some(o) = v.as_object_mut() {
            o.insert("events_total".into(), json!(total));
            o.insert("events_dropped".into(), json!(drop));
        }
    }
    v
}

// ---------------------------------------------------------------- MCP 协议

fn tools_list() -> Value {
    json!({
        "tools": [
            {"name":"editor_start","description":"启动星火编辑器（按项目组装启动命令，等待 MCP 桥上线；幂等：已在线直接返回）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string","description":"项目根，缺省取应用最近项目/在线编辑器当前地图"},"wait_online":{"type":"boolean","default":true},"timeout_ms":{"type":"integer","default":120000}}}},
            {"name":"editor_stop","description":"关闭星火编辑器（直接结束进程，不做优雅退出）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"start_debug","description":"启动调试（默认 restart_last_debug：跳过编辑器编译构建、载入最新 lua；无上一次调试版本自动回退全量；full=true 强制全量）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"full":{"type":"boolean","default":false}}}},
            {"name":"stop_debug","description":"停止调试","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"get_game_logs","description":"获取游戏日志/编辑器日志/MCP桥日志的最新文件信息。tail_lines=0（默认）只回文件信息防爆上下文；match=正则时回命中行；tail_lines>0 回末尾原文。match/tail 模式下均附 errors 段：日志中的报错行（[error]/[ERROR]）单独上浮，防前置错误导致假阳性/假阴性漏判。同条行收纳：剥离行首[时间][pid]后相同的行只回最近一次并附(×N)计数。离线可用。source：game_client/game_server/service_core/xdeditor_client/bridge_main/bridge_audit，或聚合前缀（game/bridge），或 all；缺省 game","inputSchema":{"type":"object","properties":{"source":{"type":"string","default":"game","description":"日志源 key 或聚合前缀，缺省 game"},"tail_lines":{"type":"integer","default":0,"description":"返回末尾行数，0=只返回文件信息"},"match":{"type":"string","description":"正则过滤：回命中行（同条收纳+计数，上限100条），可与 tail_lines 同用"}}}},
            {"name":"publish_project","description":"发布项目到创作者中心（分钟级耗时；danger 级默认放行，调用进审计日志）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"timeout_ms":{"type":"integer","default":600000}}}},
            {"name":"capture_game","description":"截取调试中的游戏画面/游戏截图（纯游戏画面+游戏 UI，不含编辑器界面；编辑器被遮挡/最小化均可后台截取），返回 png 路径，用 Read 查看。ratio 输出倍率（0.5/1/2/3/4）；crop={x,y,w,h} 只截视口局部（游戏视口逻辑坐标，原点=视口左上，越界自动 clamp）；max_width 输出宽度上限（默认 1280 防爆上下文，与 ratio 同给时为最终上限，传大值可放开）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"ratio":{"type":"number","default":1},"crop":{"type":"object","properties":{"x":{"type":"number"},"y":{"type":"number"},"w":{"type":"number"},"h":{"type":"number"}},"required":["x","y","w","h"]},"max_width":{"type":"integer","default":1280}}}},
            {"name":"get_status","description":"获取编辑器状态（地图路径/调试中/弹窗抑制）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"run_scenario","description":"场景脚本：把「起调试→操作 UI→验证」编成 steps 数组一次跑完，替代多次单步往返。步骤 op：invoke{id,args,timeout_ms}（调 lua.* 等桥能力）/start_debug/stop_debug/capture{ratio,crop,max_width}/logs{source,tail_lines,match}/wait{ms}/note{text}；默认遇错即停（stop_on_error=false 继续），每步结果截断 2KB","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"steps":{"type":"array","items":{"type":"object"}},"stop_on_error":{"type":"boolean","default":true}},"required":["steps"]}},
            {"name":"search_capabilities","description":"[在线透传桥 Gateway] 搜索编辑器能力（id/描述/别名/标签模糊匹配）。返回简化签名+风险级别，多数场景 search→invoke 两步完成调用；编辑器完整能力（能力目录数百条）都经此触达","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"query":{"type":"string","description":"关键词，可多个（空格分隔）"},"limit":{"type":"integer","description":"返回条数，默认 5，上限 10"}},"required":["query"]}},
            {"name":"describe_capability","description":"[在线透传桥 Gateway] 查看能力完整定义（参数 JSON Schema/返回/风险/示例/前置条件），疑难时深查","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"id":{"type":"string","description":"能力 id（search 返回的）"}},"required":["id"]}},
            {"name":"invoke_capability","description":"[在线透传桥 Gateway] 统一调用入口。参数校验失败时错误内嵌 compact schema，按提示修正后重试即可","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"id":{"type":"string"},"args":{"type":"object","description":"调用参数"},"timeout_ms":{"type":"integer","description":"超时毫秒，默认 5000"}},"required":["id"]}},
            {"name":"list_namespaces","description":"[在线透传桥 Gateway] 列出能力命名空间（svc/cpp/datacore/cmd/lua/sys）及各空间能力数","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"}}}},
            {"name":"get_events","description":"[在线透传桥 Gateway] 拉取事件缓冲中 seq > since 的事件（地图加载/调试启动/弹窗抑制/能力调用失败/danger 拒绝等）。MCP 层本地切片：默认只回最新 50 条并附 events_total/events_dropped，防长会话事件缓冲灌入；limit 可调","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"since":{"type":"integer"},"limit":{"type":"integer","default":50,"description":"最多返回最新条数（MCP 层本地切片）"}}}},
            {"name":"set_suppress","description":"[在线透传桥 Gateway] 设置编辑器弹窗抑制开关（抑制期间弹窗自动按确认/关闭处理）","inputSchema":{"type":"object","properties":{"project_path":{"type":"string"},"enabled":{"type":"boolean"}},"required":["enabled"]}}
        ]
    })
}

fn call_tool(name: &str, args: &Value) -> Result<Value> {
    let r = match name {
        "editor_start" => tool_editor_start(args),
        "editor_stop" => tool_editor_stop(args),
        "start_debug" => tool_start_debug(args),
        "stop_debug" => tool_stop_debug(args),
        "get_game_logs" => tool_get_game_logs(args),
        "publish_project" => tool_publish_project(args),
        "capture_game" => tool_capture_game(args),
        "get_status" => tool_get_status(args),
        // 场景脚本：步骤数组一次跑完（invoke/capture/logs/wait/note/start_debug/stop_debug）
        "run_scenario" => crate::scenario::run_scenario(args),
        // 桥 Gateway 元工具透传（编辑器完整能力入口）
        "search_capabilities" | "describe_capability" | "invoke_capability"
        | "list_namespaces" | "get_events" | "set_suppress" => {
            tool_bridge_meta(args, name)
        }
        _ => Err(anyhow!("unknown tool: {name}")),
    };
    r.map(|v| with_hint(name, v, args))
}

fn result_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn result_err(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// R1.1 统一响应截断护栏：tool_result 序列化后超 32KB 即截断 + 重试引导（一处兜住全部工具）
const RESULT_CAP: usize = 32 * 1024;

fn truncate_guard(text: String) -> String {
    if text.len() <= RESULT_CAP {
        return text;
    }
    let mut end = RESULT_CAP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n...[响应已截断：原始 {} 字节 > 上限 {} 字节。请用更精确参数重试（缩小范围/加过滤条件/分次查询）]",
        &text[..end],
        text.len(),
        RESULT_CAP
    )
}

/// R1.5 响应引导：成功响应附一行 hint（可选、不重复 description 已有内容、
/// 不引用尚未上线的能力——hint 文本随能力集同步发版）。
fn with_hint(tool: &str, mut v: Value, args: &Value) -> Value {
    let hint = match tool {
        "editor_start" => Some("下一步：start_debug 启动调试；或 search_capabilities 搜索编辑器能力"),
        "start_debug" => Some(
            "验证画面用 capture_game 截图；排障用 get_game_logs（先 tail_lines=0 看文件信息，再用 match 过滤关键行）",
        ),
        "capture_game" => {
            Some("用 Read 查看 png；只看局部可传 crop={\"x\",\"y\",\"w\",\"h\"}（视口逻辑坐标）重截")
        }
        "get_game_logs" => Some(if args.get("match").and_then(|v| v.as_str()).is_some() {
            "未命中或需上下文时可调宽正则，或按返回的 path 自行读取原文"
        } else if args.get("tail_lines").and_then(|v| v.as_u64()).unwrap_or(0) > 0 {
            "只看关键行可用 match 参数按正则过滤（返回命中行+行号，上限 100 行）"
        } else {
            "传 tail_lines 看末尾内容，或用 match 正则过滤关键行（如 ERROR）"
        }),
        "get_status" => Some("调试闭环：start_debug 起局 → capture_game 看画面 → get_game_logs 排障"),
        "search_capabilities" => Some("下一步 describe_capability 看参数定义，确认后 invoke_capability 调用"),
        "describe_capability" => Some("确认参数后用 invoke_capability {id, args} 调用"),
        "invoke_capability" => Some("界面调试闭环：lua.find_ui 定位 → lua.click_ui/lua.input_text 操作 → capture_game(crop) 验证"),
        "list_namespaces" => Some("用 search_capabilities 按关键词搜索具体能力"),
        "get_events" => Some("增量拉取传 since=<上次返回的 latest>；事件多时已按 limit 截取最新若干条"),
        _ => None,
    };
    if let (Some(h), Some(o)) = (hint, v.as_object_mut()) {
        o.insert("hint".into(), json!(h));
    }
    v
}

fn tool_result(id: &Value, r: Result<Value>) -> Value {
    match r {
        Ok(v) => result_ok(
            id,
            json!({
                "content": [{ "type": "text", "text": truncate_guard(serde_json::to_string_pretty(&v).unwrap_or_default()) }]
            }),
        ),
        Err(e) => result_ok(
            id,
            json!({
                "content": [{ "type": "text", "text": truncate_guard(serde_json::to_string_pretty(&json!({"ok":false,"error":format!("{e:#}")})).unwrap_or_default()) }],
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
        // 管道首行可能带 BOM（PowerShell 等管道编码前导），剥离再解析
        let line = line.trim().trim_start_matches('\u{feff}');
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_guard() {
        // 短文本原样
        let short = "abc中文".to_string();
        assert_eq!(truncate_guard(short.clone()), short);
        // 超长截断 + 引导提示；截断点落在 UTF-8 字符边界
        let big = "中".repeat(RESULT_CAP); // 3 字节/字，必超上限
        let out = truncate_guard(big);
        assert!(out.contains("响应已截断"));
        assert!(out.contains("请用更精确参数重试"));
        assert!(out.len() <= RESULT_CAP + 512);
    }

    #[test]
    fn test_slice_events() {
        // 超上限：留最新 limit 条 + 总数/丢弃数
        let events: Vec<Value> = (1..=60).map(|i| json!({"seq": i})).collect();
        let v = slice_events(json!({"events": events, "latest": 60}), 50);
        let arr = v["events"].as_array().unwrap();
        assert_eq!(arr.len(), 50);
        assert_eq!(arr[0]["seq"].as_u64(), Some(11));
        assert_eq!(v["events_total"].as_u64(), Some(60));
        assert_eq!(v["events_dropped"].as_u64(), Some(10));
        assert_eq!(v["latest"].as_u64(), Some(60));
        // 未超上限原样
        let small = json!({"events": [{"seq": 1}], "latest": 1});
        let v2 = slice_events(small, 50);
        assert_eq!(v2["events"].as_array().unwrap().len(), 1);
        assert!(v2.get("events_total").is_none());
        // 非预期结构不崩
        let v3 = slice_events(json!({"foo": 1}), 50);
        assert_eq!(v3["foo"].as_u64(), Some(1));
    }

    #[test]
    fn test_with_hint() {
        let v = with_hint("editor_start", json!({"ok": true}), &json!({}));
        assert!(v["hint"].as_str().unwrap().contains("start_debug"));
        // 无 hint 的工具原样
        let v2 = with_hint("editor_stop", json!({"ok": true}), &json!({}));
        assert!(v2.get("hint").is_none());
        // get_game_logs 按参数选 hint
        let v3 = with_hint("get_game_logs", json!({}), &json!({"match": "ERROR"}));
        assert!(v3["hint"].as_str().unwrap().contains("正则"));
        // 非对象结果不崩
        let v4 = with_hint("editor_start", Value::Null, &json!({}));
        assert!(v4.is_null());
    }
}
