//! 编辑器生命周期与日志（0.5.4 自 bgd_sce_tools 迁入：应用自持编辑器控制能力）。
//!
//! - editor_start / editor_stop：星火编辑器进程启停（启动命令形态已实证）
//! - get_logs：读 <运行根>/logs/ 下游戏客户端/服务端/bgd_csharp 最新日志文件信息（离线可用）
//! - 应用设置（editor_exe_name 等）存 <运行根>/logs/bgd_csharp/config.json（与 mcp_port 同文件）

use super::bridge_client;
use super::locate;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 默认编辑器 exe 名（应用设置 editor_exe_name 可覆盖，防用户改名）
pub const DEFAULT_EDITOR_EXE: &str = "星火编辑器.exe";

// ---------------------------------------------------------------- 应用设置（config.json）

/// 配置文件：<运行根>/logs/bgd_csharp/config.json（与 mcp_port 同文件）
fn config_path(engine_root: &Path) -> PathBuf {
    engine_root.join("logs").join("bgd_csharp").join("config.json")
}

fn read_config(engine_root: &Path) -> Value {
    std::fs::read_to_string(config_path(engine_root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(json!({}))
}

/// 读编辑器 exe 名（缺省默认）
pub fn editor_exe_name(engine_root: &Path) -> String {
    read_config(engine_root)["editor_exe_name"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_EDITOR_EXE)
        .to_string()
}

/// 写编辑器 exe 名（保留文件内其他键）
pub fn set_editor_exe_name(engine_root: &Path, name: &str) -> Result<(), String> {
    let path = config_path(engine_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let mut cfg = read_config(engine_root);
    cfg["editor_exe_name"] = Value::String(name.to_string());
    std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap())
        .map_err(|e| format!("写入配置失败: {e}"))
}

// ---------------------------------------------------------------- 应用级配置（bgd_appsdk 统一实现，exe 旁 <app>.config.json）

/// 最近项目路径（GUI 选择项目时写入；MCP/CLI 缺省项目用它）
pub fn last_project_path() -> Option<PathBuf> {
    bgd_appsdk::config::last_project_path().filter(|p| locate::is_valid_project(p))
}

/// 写最近项目路径
pub fn set_last_project_path(project_root: &Path) {
    bgd_appsdk::config::set_last_project_path(project_root);
}

// ---------------------------------------------------------------- editor_start / editor_stop

/// 启动星火编辑器并等待 MCP 桥上线。幂等：已在线直接返回现状。
pub fn editor_start(
    project_root: &Path,
    wait_online: bool,
    timeout_ms: u64,
) -> Result<Value, String> {
    let target = locate::locate(project_root)?;
    let engine_root = target.engine_root()?;
    let exe_name = editor_exe_name(&engine_root);

    // 幂等：已在线
    if let Some(port) = bridge_client::online_port(&engine_root) {
        return Ok(json!({
            "already_running": true,
            "port": port,
            "mcp_url": format!("http://127.0.0.1:{port}/mcp"),
        }));
    }

    let exe = engine_root.join(&exe_name);
    if !exe.is_file() {
        return Err(format!(
            "编辑器 exe 不存在: {}（可在「编辑器补丁」设置页修改编辑器 exe 名）",
            exe.display()
        ));
    }

    let sce = project_root.join("project.sce");
    let started = Instant::now();
    let child = std::process::Command::new(&exe)
        .arg("-inner")
        .arg("-winui_material_editor")
        .arg("-winui_resource_store")
        .arg(format!("-editor_api_version={}", target.api_version))
        .arg(format!("-file_path={}", sce.display()))
        .spawn()
        .map_err(|e| format!("启动编辑器失败 {}: {e}", exe.display()))?;
    let pid = child.id();

    if !wait_online {
        return Ok(json!({ "started": true, "pid": pid, "wait_online": false }));
    }

    // 等待桥上线：port 文件出现 + 握手成功
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(port) = bridge_client::online_port(&engine_root) {
            return Ok(json!({
                "started": true,
                "pid": pid,
                "port": port,
                "mcp_url": format!("http://127.0.0.1:{port}/mcp"),
                "elapsed_ms": started.elapsed().as_millis() as u64,
            }));
        }
        if Instant::now() >= deadline {
            return Ok(json!({
                "started": true,
                "pid": pid,
                "mcp_online": false,
                "warning": "编辑器进程已拉起但 MCP 桥超时未上线。排查：bgd_mcp_bridge 补丁模块是否启用；编辑器升级是否覆盖了补丁（打开「编辑器补丁」应用检查）；日志见 <运行根>/logs/bgd_csharp/",
                "elapsed_ms": started.elapsed().as_millis() as u64,
            }));
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

/// 关闭星火编辑器：直接结束进程（定稿不做优雅退出，避免保存确认弹窗挂住）。
pub fn editor_stop(project_root: &Path) -> Result<Value, String> {
    let target = locate::locate(project_root)?;
    let engine_root = target.engine_root()?;

    // 取 pid：在线走 server_info（exe 是启动器，真实编辑器是另一个进程，必须以桥为准）；
    // 离线按 exe 路径匹配进程
    let pid = bridge_client::online_port(&engine_root)
        .and_then(|port| {
            bridge_client::bridge_rpc(port, "server_info", json!({}), 10_000).ok()?["pid"]
                .as_u64()
                .map(|p| p as u32)
        })
        .or_else(|| find_editor_pid(&engine_root.join(editor_exe_name(&engine_root))));

    let Some(pid) = pid else {
        return Ok(json!({ "stopped": false, "message": "编辑器未在运行" }));
    };

    // /F 强杀（不带 /F 的 WM_CLOSE 会触发保存确认弹窗）；/T 连带子进程
    let out = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|e| format!("执行 taskkill 失败: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(format!("taskkill 失败: {stderr}{stdout}"));
    }

    // 等进程消失（最多 15s）
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let check = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        if let Ok(o) = check {
            let s = String::from_utf8_lossy(&o.stdout);
            if !s.contains(&pid.to_string()) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(json!({ "stopped": true, "pid": pid }))
}

/// 按 exe 全路径找进程 pid（Win32_Process ExecutablePath 匹配，兼容大小写与斜杠）。
/// editor_stop 离线兜底与 capture 截图共用（单一实现）。
pub fn find_editor_pid(exe_path: &Path) -> Option<u32> {
    let want = exe_path.display().to_string().replace('/', "\\").to_lowercase();
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process | Select-Object ProcessId,ExecutablePath | ConvertTo-Json -Compress",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let doc: Value = serde_json::from_str(&text).ok()?;
    let list = match &doc {
        Value::Array(a) => a.clone(),
        Value::Object(_) => vec![doc],
        _ => return None,
    };
    for p in list {
        let exe = p["ExecutablePath"].as_str().unwrap_or("").to_lowercase();
        if exe == want {
            return p["ProcessId"].as_u64().map(|v| v as u32);
        }
    }
    None
}

// ---------------------------------------------------------------- get_game_logs

/// 日志源定义：(key, 子目录, 文件前缀, 说明) —— 每类取最新一个文件（0.5.6 起带 desc 说明）
const LOG_SOURCES: &[(&str, &str, &str, &str)] = &[
    ("bridge_audit", "bgd_csharp", "audit-", "MCP桥审计日志（编辑器补丁 bgd_mcp_bridge 的 write/danger 能力调用审计）"),
    ("bridge_main", "bgd_csharp", "bgd_csharp-", "MCP桥入口日志（编辑器补丁 bgd_mcp_bridge 服务启动/请求处理日志）"),
    ("xdeditor_client", "lua", "lua-editor-", "编辑器日志（星火编辑器操作动作及运行日志）"),
    ("game_client", "lua", "lua-game-", "游戏客户端日志（调试游戏后产生）"),
    ("service_core", "server", "core-game-server-", "服务器底层日志（游戏服务端底层框架日志）"),
    ("game_server", "server", "lua-game-server-", "游戏服务端日志（游戏服务端自身代码日志）"),
];

/// 对外输出路径统一正斜杠（0.5.6 R2）
pub fn to_slash(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// 获取游戏日志（离线可用；默认只返回文件路径与信息，tail_lines>0 才带内容）。
/// source 取值：具体 key（见 LOG_SOURCES）/ 动态聚合前缀（如 bridge 命中全部 bridge_*）/
/// all / 缺省 game（命中 game_client + game_server）。
pub fn get_game_logs(project_root: &Path, source: &str, tail_lines: usize) -> Result<Value, String> {
    let target = locate::locate(project_root)?;
    let logs_root = target.engine_root()?.join("logs");

    let source = if source.trim().is_empty() { "game" } else { source.trim() };
    // 匹配规则：all=全部；精确 key；否则动态聚合（key 以 `source_` 为前缀）
    let matched: Vec<&(&str, &str, &str, &str)> = LOG_SOURCES
        .iter()
        .filter(|(key, _, _, _)| {
            source == "all" || *key == source || key.starts_with(&format!("{source}_"))
        })
        .collect();
    if matched.is_empty() {
        let keys: Vec<&str> = LOG_SOURCES.iter().map(|(k, _, _, _)| *k).collect();
        return Err(format!(
            "source '{source}' 未匹配到任何日志项（可用 key：{}；也可用前缀聚合如 bridge/game、或 all）",
            keys.join(" / ")
        ));
    }

    let mut out = serde_json::Map::new();
    for (key, sub, prefix, desc) in matched {
        let dir = logs_root.join(sub);
        match latest_file(&dir, prefix) {
            Some(p) => {
                out.insert(key.to_string(), file_info(&p, desc, tail_lines));
            }
            None => {
                out.insert(
                    key.to_string(),
                    json!({
                        "desc": desc,
                        "path": Value::Null,
                        "note": format!("{} 下无 {prefix}*.log（未产生过该类日志）", to_slash(&dir)),
                    }),
                );
            }
        }
    }
    Ok(json!({ "logs_root": to_slash(&logs_root), "logs": out }))
}

/// 单个日志文件信息
fn file_info(path: &Path, desc: &str, tail_lines: usize) -> Value {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let to_local = |t: Option<std::time::SystemTime>| -> Value {
        t.map(|t| {
            let secs = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64
                + 8 * 3600;
            let days = secs / 86400;
            let rem = secs % 86400;
            let (y, m, d) = bgd_appsdk::log::civil_from_days(days);
            Value::String(format!(
                "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
                rem / 3600,
                (rem % 3600) / 60,
                rem % 60
            ))
        })
        .unwrap_or(Value::Null)
    };
    let created = meta.as_ref().and_then(|m| m.created().ok());
    let modified = meta.as_ref().and_then(|m| m.modified().ok());

    let lines = count_lines(path).unwrap_or(0);

    let mut info = json!({
        "desc": desc,
        "path": to_slash(path),
        "size": size,
        "created": to_local(created),
        "modified": to_local(modified),
        "lines": lines,
    });
    if tail_lines > 0 {
        const TAIL_BYTE_CAP: usize = 64 * 1024;
        let (tail, truncated) = read_tail(path, tail_lines, TAIL_BYTE_CAP);
        info["tail"] = Value::String(tail);
        info["truncated"] = Value::Bool(truncated);
    }
    info
}

fn count_lines(path: &Path) -> Option<usize> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::with_capacity(256 * 1024, f);
    let mut n = 0;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => n += 1,
            Err(_) => break,
        }
    }
    Some(n)
}

/// 读文件末尾 N 行（带字节上限；从尾部窗口读避免整文件载入）
fn read_tail(path: &Path, tail_lines: usize, byte_cap: usize) -> (String, bool) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return (String::new(), false);
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(byte_cap as u64);
    let truncated = start > 0;
    if f.seek(SeekFrom::Start(start)).is_err() {
        return (String::new(), false);
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return (String::new(), false);
    }
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    let lines = if truncated && !lines.is_empty() {
        &lines[1..]
    } else {
        &lines[..]
    };
    let tail: Vec<&str> = lines.iter().rev().take(tail_lines).rev().cloned().collect();
    (tail.join("\n"), truncated)
}

/// 目录下按前缀找最新文件（mtime 优先）
fn latest_file(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e == "log").unwrap_or(false)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(prefix))
                    .unwrap_or(false)
        })
        .max_by_key(|p| {
            p.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_from_days() {
        // 算法单一真相在 bgd_appsdk::log（此处守护调用约定的换算结果）
        assert_eq!(bgd_appsdk::log::civil_from_days(0), (1970, 1, 1));
        assert_eq!(bgd_appsdk::log::civil_from_days(20270), (2025, 7, 1));
    }

    #[test]
    fn test_tail_and_count() {
        let dir = std::env::temp_dir().join(format!("bgd_logs_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("lua-game-20260101.log");
        let content: String = (1..=100).map(|i| format!("line{i}\n")).collect();
        std::fs::write(&f, &content).unwrap();
        assert_eq!(count_lines(&f), Some(100));
        let (tail, truncated) = read_tail(&f, 3, 64 * 1024);
        assert!(!truncated);
        assert_eq!(tail, "line98\nline99\nline100");
        let (tail2, truncated2) = read_tail(&f, 3, 64);
        assert!(truncated2);
        assert!(tail2.contains("line100"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_latest_file() {
        let dir = std::env::temp_dir().join(format!("bgd_logs_latest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lua-game-1.log"), "a").unwrap();
        std::fs::write(dir.join("core-game-server-1.log"), "b").unwrap();
        std::fs::write(dir.join("other.txt"), "c").unwrap();
        let got = latest_file(&dir, "lua-game-").unwrap();
        assert!(got.ends_with("lua-game-1.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
