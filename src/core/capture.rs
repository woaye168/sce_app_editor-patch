//! 调试游戏画面截取（0.5.4 自 bgd_sce_tools 迁入并定稿）。
//!
//! 终版方案（真机验证）：
//! 1. lua 桥 `lua.get_game_view_rect` 读 PIE 视口控件（base.ui 树 viewport 控件）的
//!    `get_screen_rect()`（引擎 UI 逻辑坐标）+ `common.get_resolution()` 逻辑分辨率；
//! 2. WGC 截取编辑器主窗口（WinUIDesktopWin32WindowClass；SDL 内容窗口实测不可直接 WGC；
//!    WinUI 主窗口呈现帧含完整合成画面，且窗口截取在遮挡/后台时仍可用）；
//! 3. 帧像素坐标系闭环裁剪：逻辑坐标与帧像素等比（s = 帧宽/逻辑宽），
//!    内容区在帧内底对齐（origin_y = 帧高 − 逻辑高×s；origin_x = (帧宽 − 逻辑宽×s)/2）；
//! 4. ratio 倍率：裁剪后用 image crate 重采样（Lanczos3）放大/缩小输出。
//!
//! 已知边界：编辑器窗口最小化时无帧可截（遮挡可以，最小化不行）。

use super::bridge_client;
use super::locate;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// 截取调试游戏画面（纯游戏画面+游戏 UI，不含编辑器界面）。
/// ratio：输出倍率（0.5/1/2/3/4，对应编辑器调试视图的倍率选项）。
/// out：自定义输出路径（缺省 <项目>/.bgd/log/screenshots/capture_<时间戳>.png）
#[cfg(windows)]
pub fn capture_game(project_root: &Path, ratio: f64, out: Option<&Path>) -> Result<Value> {
    capture_game_impl(project_root, ratio, out, false)
}

/// open_explorer=true 时截图完成后用资源管理器选中文件（编辑器内拍照按钮路径用，
/// 替代官方「截图后打开所在文件夹」行为——由 CLI 进程自己做，不依赖编辑器 lua 计时器）
#[cfg(windows)]
pub fn capture_game_impl(project_root: &Path, ratio: f64, out: Option<&Path>, open_explorer: bool) -> Result<Value> {
    let target = locate::locate(project_root).map_err(|e| anyhow!(e))?;
    let engine_root = target.engine_root();
    let port = bridge_client::online_port(&engine_root)
        .ok_or_else(|| anyhow!("编辑器不在线（MCP 桥不可达）。请先 editor_start 启动编辑器"))?;

    // 1. lua 桥取 PIE 视口逻辑矩形 + 逻辑分辨率
    let rect = bridge_client::bridge_invoke(port, "lua.get_game_view_rect", json!({}), 15_000)?;
    let rx = rect["x"].as_f64().unwrap_or(0.0);
    let ry = rect["y"].as_f64().unwrap_or(0.0);
    let rw = rect["width"].as_f64().unwrap_or(0.0);
    let rh = rect["height"].as_f64().unwrap_or(0.0);
    let lw = rect["logical_width"].as_f64().unwrap_or(0.0);
    let lh = rect["logical_height"].as_f64().unwrap_or(0.0);
    if rw < 10.0 || rh < 10.0 || lw < 1.0 || lh < 1.0 {
        return Err(anyhow!("游戏视口矩形异常: {rect}（游戏未在调试？）"));
    }

    // 2. 找编辑器主窗口（排除我们的 bgd_mcp_bridge 隐藏窗口——同类名，按标题排除）。
    //    实测结论：WinUI 主窗口有重定向表面，WGC 可截完整合成画面（含 re-parent 进去的
    //    SDL 内容与游戏画面）；SDL 窗口直接呈现，WGC 截出黑图。WGC 对隐藏/最小化窗口
    //    创建 GraphicsCaptureItem 会失败——此时先不激活地恢复窗口（SW_SHOWNOACTIVATE），
    //    截完恢复原状态（遮挡覆盖不影响截取，这才是「后台截图」的正确形态）。
    let pid = bridge_client::bridge_rpc(port, "server_info", json!({}), 10_000)
        .ok()
        .and_then(|v| v["pid"].as_u64())
        .map(|p| p as u32)
        .or_else(|| find_editor_pid(&engine_root))
        .ok_or_else(|| anyhow!("找不到编辑器进程"))?;
    let main = find_window_by_class(pid, "WinUIDesktopWin32WindowClass", Some("bgd_mcp_bridge"))
        .ok_or_else(|| anyhow!("找不到编辑器主窗口（pid={pid}）"))?;
    let _restore = WindowRestoreGuard::ensure_visible(main.hwnd);

    // 3. WGC 截主窗口 + 帧内裁剪 + 倍率重采样
    let path = match out {
        Some(p) => p.to_path_buf(),
        None => {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            project_root
                .join(".bgd")
                .join("log")
                .join("screenshots")
                .join(format!("capture_{ts}.png"))
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let map = ViewportMap {
        rect_logical: (rx, ry, rw, rh),
        logical_res: (lw, lh),
        ratio,
    };
    let (cw, ch) = wgc_capture_mapped(main.hwnd, &path, &map)?;

    if open_explorer {
        let p = path.display().to_string().replace('/', "\\");
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{p}"))
            .spawn();
    }

    Ok(json!({
        "path": path.display().to_string(),
        "width": cw,
        "height": ch,
        "ratio": ratio,
        "mode": "game_viewport",
    }))
}

#[cfg(not(windows))]
pub fn capture_game(_project_root: &Path, _ratio: f64, _out: Option<&Path>) -> Result<Value> {
    Err(anyhow!("仅支持 Windows"))
}

/// 编辑器进程 pid（离线兜底：按 exe 路径匹配）
#[cfg(windows)]
fn find_editor_pid(engine_root: &Path) -> Option<u32> {
    let exe = engine_root
        .join(super::editor::editor_exe_name(engine_root))
        .display()
        .to_string()
        .replace('/', "\\")
        .to_lowercase();
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
        if p["ExecutablePath"].as_str().unwrap_or("").to_lowercase() == exe {
            return p["ProcessId"].as_u64().map(|v| v as u32);
        }
    }
    None
}

/// 视口映射参数（引擎逻辑坐标 + 倍率）
#[cfg(windows)]
#[derive(Clone)]
struct ViewportMap {
    rect_logical: (f64, f64, f64, f64),
    logical_res: (f64, f64),
    ratio: f64,
}

/// 编辑器窗口信息
#[cfg(windows)]
struct WindowInfo {
    hwnd: *mut std::ffi::c_void,
}

/// 窗口可见性守护：隐藏/最小化时先不激活地恢复，Drop 时还原原状态
#[cfg(windows)]
struct WindowRestoreGuard {
    hwnd: *mut std::ffi::c_void,
    /// 原状态：0=原本可见（无需还原） 2=最小化（还原为最小化） 1=隐藏（重新隐藏）
    restore_as: i32,
}

#[cfg(windows)]
impl WindowRestoreGuard {
    fn ensure_visible(hwnd: *mut std::ffi::c_void) -> Self {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsIconic, IsWindowVisible, ShowWindow, SW_RESTORE,
        };
        let mut restore_as = 0;
        unsafe {
            let visible = IsWindowVisible(hwnd as HWND) != 0;
            let iconic = IsIconic(hwnd as HWND) != 0;
            if iconic {
                restore_as = 2;
            } else if !visible {
                restore_as = 1;
            }
            if restore_as != 0 {
                // SW_RESTORE 从最小化/隐藏恢复（不抢焦点）
                ShowWindow(hwnd as HWND, SW_RESTORE);
                // 实测：恢复后引擎重排版+首帧呈现需要 ~2s，等太短会截到黑图
                std::thread::sleep(Duration::from_millis(2500));
            }
        }
        Self { hwnd, restore_as }
    }
}

#[cfg(windows)]
impl Drop for WindowRestoreGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_MINIMIZE};
        unsafe {
            match self.restore_as {
                2 => {
                    ShowWindow(self.hwnd as HWND, SW_MINIMIZE);
                }
                1 => {
                    ShowWindow(self.hwnd as HWND, SW_HIDE);
                }
                _ => {}
            }
        }
    }
}

/// 枚举该 pid 的顶层窗口，取指定类名且面积最大者；skip_title 排除同类自家窗口。
/// 注意 SDL 窗口 IsWindowVisible 可能报 False（引擎自绘边框技巧），不做可见性过滤。
#[cfg(windows)]
fn find_window_by_class(pid: u32, class: &str, skip_title: Option<&str>) -> Option<WindowInfo> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    };

    struct Ctx {
        pid: u32,
        class: String,
        skip_title: String,
        best_hwnd: HWND,
        best_area: i64,
    }
    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let ctx = &mut *(lparam as *mut Ctx);
        let mut wpid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut wpid);
        if wpid != ctx.pid {
            return 1;
        }
        let mut cls = [0u16; 64];
        let n = GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32);
        if String::from_utf16_lossy(&cls[..n as usize]) != ctx.class {
            return 1;
        }
        if !ctx.skip_title.is_empty() {
            let mut t = [0u16; 128];
            let m = GetWindowTextW(hwnd, t.as_mut_ptr(), t.len() as i32);
            if String::from_utf16_lossy(&t[..m as usize]) == ctx.skip_title {
                return 1;
            }
        }
        let mut rc = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut rc) != 0 {
            let area = (rc.right - rc.left) as i64 * (rc.bottom - rc.top) as i64;
            if area > ctx.best_area {
                ctx.best_area = area;
                ctx.best_hwnd = hwnd;
            }
        }
        1
    }

    let mut ctx = Ctx {
        pid,
        class: class.to_string(),
        skip_title: skip_title.unwrap_or("").to_string(),
        best_hwnd: std::ptr::null_mut(),
        best_area: 0,
    };
    unsafe {
        EnumWindows(Some(enum_proc), &mut ctx as *mut Ctx as LPARAM);
    }
    if ctx.best_hwnd.is_null() {
        return None;
    }
    Some(WindowInfo { hwnd: ctx.best_hwnd })
}

/// WGC 截窗口 + 帧内换算裁剪 + 倍率重采样。返回输出图 (宽, 高)。
#[cfg(windows)]
fn wgc_capture_mapped(
    hwnd: *mut std::ffi::c_void,
    path: &Path,
    map: &ViewportMap,
) -> Result<(u32, u32)> {
    use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
    use windows_capture::frame::Frame;
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    struct CapFlags {
        map: ViewportMap,
        done: std::sync::mpsc::Sender<Result<(u32, u32, image::RgbaImage), String>>,
    }
    struct CapHandler {
        map: ViewportMap,
        done: std::sync::mpsc::Sender<Result<(u32, u32, image::RgbaImage), String>>,
    }
    impl GraphicsCaptureApiHandler for CapHandler {
        type Flags = CapFlags;
        type Error = anyhow::Error;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let flags = ctx.flags;
            Ok(Self { map: flags.map, done: flags.done })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let r = (|| {
                // 帧像素坐标系闭环（实测校准）：逻辑坐标与帧像素等比（s = 帧宽/逻辑宽），
                // 内容区在帧内底对齐（顶部富裕 = WinUI 标题栏，非引擎内容区）
                let s = frame.width() as f64 / self.map.logical_res.0;
                let origin_x = (frame.width() as f64 - self.map.logical_res.0 * s) / 2.0;
                let origin_y = frame.height() as f64 - self.map.logical_res.1 * s;
                let (rx, ry, rw, rh) = self.map.rect_logical;
                let cx = (origin_x + rx * s).round().max(0.0) as u32;
                let cy = (origin_y + ry * s).round().max(0.0) as u32;
                let cw = ((rw * s).round() as u32).min(frame.width().saturating_sub(cx));
                let ch = ((rh * s).round() as u32).min(frame.height().saturating_sub(cy));
                if cw < 10 || ch < 10 {
                    return Err(format!(
                        "裁剪框异常（{cx},{cy},{cw}x{ch}，帧 {}x{}）",
                        frame.width(),
                        frame.height()
                    ));
                }
                let mut buf = frame
                    .buffer_crop(cx, cy, cx + cw, cy + ch)
                    .map_err(|e| format!("裁剪帧缓冲失败: {e}"))?;
                let raw = buf
                    .as_nopadding_buffer()
                    .map_err(|e| format!("读取帧数据失败: {e}"))?;
                let img = image::RgbaImage::from_raw(cw, ch, raw.to_vec())
                    .ok_or_else(|| "构造图像失败".to_string())?;
                Ok((cw, ch, img))
            })();
            let _ = self.done.send(r);
            let _ = capture_control.stop();
            Ok(())
        }
    }

    let window = Window::from_raw_hwnd(hwnd);
    let (tx, rx) = std::sync::mpsc::channel();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::Default,
        DrawBorderSettings::Default,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Rgba8,
        CapFlags { map: map.clone(), done: tx },
    );
    CapHandler::start(settings).map_err(|e| anyhow!("启动窗口捕获失败: {e}"))?;
    let (cw, ch, img) = rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| anyhow!("窗口捕获超时（15s 未收到帧；窗口可能最小化）"))?
        .map_err(|e| anyhow!(e))?;

    // 倍率重采样（≈1 时原样保存）
    let ratio = map.ratio;
    let (out_img, ow, oh) = if (ratio - 1.0).abs() < 0.01 {
        (img, cw, ch)
    } else {
        let nw = ((cw as f64 * ratio).round() as u32).max(1);
        let nh = ((ch as f64 * ratio).round() as u32).max(1);
        (
            image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3),
            nw,
            nh,
        )
    };
    // 快速 PNG 编码（截图场景对体积不敏感，Fast+NoFilter 显著快于默认）
    {
        use image::codecs::png::{CompressionType, FilterType, PngEncoder};
        use image::ImageEncoder;
        let file = std::fs::File::create(path).map_err(|e| anyhow!("创建截图文件失败: {e}"))?;
        let mut w = std::io::BufWriter::new(file);
        let enc = PngEncoder::new_with_quality(&mut w, CompressionType::Fast, FilterType::NoFilter);
        enc.write_image(&out_img, ow, oh, image::ExtendedColorType::Rgba8)
            .map_err(|e| anyhow!("保存截图失败: {e}"))?;
    }
    Ok((ow, oh))
}
