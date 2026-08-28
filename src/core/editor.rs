//! 编辑器生命周期（0.5.4 自 bgd_sce_tools 迁入：应用自持编辑器控制能力）。
//!
//! - editor_start / editor_stop：星火编辑器进程启停（启动命令形态已实证）
//! - 日志读取在 core/logs.rs（0.8.0 拆出；此处 re-export 保持 editor::get_game_logs 调用不变）
//! - 应用设置（editor_exe_name 等）存 <运行根>/logs/bgd_csharp/config.json（与 mcp_port 同文件）

use super::bridge_client;
use super::locate;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 日志读取模块 re-export（0.8.0 拆分后保持旧调用点 editor::get_game_logs / editor::to_slash 不变）
pub use crate::core::logs::{get_game_logs, to_slash};

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

