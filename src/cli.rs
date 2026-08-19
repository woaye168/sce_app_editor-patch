//! CLI 子命令（0.5.4 起：编辑器控制能力随应用自持，与 bgd_sce_tools 解耦）。
//!
//! 用法：
//!   sce_app_editor-patch editor start|stop [--project-path <项目根>] [--no-wait]
//!   sce_app_editor-patch logs [client|server|bridge|all] [行数] [--project-path <项目根>]
//!   sce_app_editor-patch capture [--ratio <倍率>] [--project-path <项目根>]
//!   sce_app_editor-patch mcp        # stdio MCP 聚合服务（AI 客户端配置入口）

use crate::core::{capture, editor};
use std::path::PathBuf;

const USAGE: &str = "
sce_app_editor-patch CLI

用法: sce_app_editor-patch <子命令> [选项]

子命令:
  editor start           启动星火编辑器（等待 MCP 桥上线；幂等）
  editor stop            关闭星火编辑器（直接结束进程）
  logs [源] [行数]       最新日志文件信息（源: client/server/bridge/all；行数 0=不取内容）
  capture [--ratio N]    截取调试游戏画面（纯游戏画面+游戏UI；倍率 0.5/1/2/3/4）
  mcp                    启动 stdio MCP 聚合服务（AI 客户端配置入口）

选项:
  --project-path <路径>  项目根目录（缺省取应用最近项目）
";

fn parse_project(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|w| w[0] == "--project-path")
        .map(|w| PathBuf::from(&w[1]))
}

fn parse_flag(args: &[String], key: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == key).map(|w| w[1].clone())
}

fn resolve_project(args: &[String]) -> Result<PathBuf, String> {
    parse_project(args)
        .or_else(editor::last_project_path)
        .ok_or_else(|| "缺少项目路径（--project-path，或先在应用内选择过项目）".to_string())
}

/// 执行 CLI 子命令，返回进程退出码
pub fn run(args: &[String]) -> i32 {
    let Some(cmd) = args.first() else {
        eprintln!("{USAGE}");
        return 2;
    };
    let rest = &args[1..];
    let result: Result<serde_json::Value, String> = match cmd.as_str() {
        "editor" => {
            let sub = rest.first().map(|s| s.as_str()).unwrap_or("");
            match sub {
                "start" => resolve_project(rest).and_then(|p| {
                    let no_wait = rest.iter().any(|a| a == "--no-wait");
                    editor::editor_start(&p, !no_wait, 120_000)
                }),
                "stop" => resolve_project(rest).and_then(|p| editor::editor_stop(&p)),
                _ => Err(format!("未知 editor 子命令: {sub}（start/stop）")),
            }
        }
        "logs" => {
            // 位置参数（剔除 --flag 及其值）依次为 source、行数
            let mut positional: Vec<&String> = Vec::new();
            let mut skip_next = false;
            for a in rest {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if a.starts_with("--") {
                    skip_next = true; // --project-path/--log 等带值
                    continue;
                }
                positional.push(a);
            }
            let source = positional.first().map(|s| s.as_str()).unwrap_or("");
            let tail: usize = positional.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            resolve_project(rest).and_then(|p| editor::get_game_logs(&p, &source, tail))
        }
        "capture" => {
            let ratio: f64 = parse_flag(rest, "--ratio")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let out = parse_flag(rest, "--out").map(PathBuf::from);
            let open_explorer = rest.iter().any(|a| a == "--open-explorer");
            resolve_project(rest).and_then(|p| {
                capture::capture_game_impl(&p, ratio, out.as_deref(), open_explorer)
                    .map_err(|e| format!("{e:#}"))
            })
        }
        _ => {
            eprintln!("未知子命令: {cmd}\n{USAGE}");
            return 2;
        }
    };
    match result {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("错误: {e}");
            1
        }
    }
}
