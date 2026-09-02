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
//! 窗口最小化/隐藏时由 WindowRestoreGuard 离屏恢复（SHOWNOACTIVATE + 屏外坐标，
//! 截完按 placement 还原）后正常截取；遮挡无需处理。

use super::bridge_client;
use super::locate;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// open_explorer=true 时截图完成后用资源管理器选中文件（编辑器内拍照按钮路径用，
/// 替代官方「截图后打开所在文件夹」行为——由 CLI 进程自己做，不依赖编辑器 lua 计时器）
#[cfg(windows)]
pub fn capture_game_impl(project_root: &Path, ratio: f64, out: Option<&Path>, open_explorer: bool) -> Result<Value> {
    do_capture(project_root, ratio, out, open_explorer, None, None, None, true, 200)
}

/// MCP 工具层专用（0.8.0 R1.2）：视口裁剪后叠加逻辑坐标子矩形裁剪 + max_width 上限，
/// 护住 AI 上下文。crop = (x, y, w, h) 游戏视口逻辑坐标（原点 (0,0)=视口左上，
/// 与 lua.get_game_view_rect / find_ui 同系），越界自动 clamp 不报错。
/// CLI/GUI/pie_capture 拍照路径走 capture_game_impl，不受此影响（用户要原图）。
/// 0.8.7：player 多人定向——player 给定（或多人局缺省=1 号玩家）时内部编排
/// mp_switch 切焦 → 等帧（wait_ms）→ WGC → 按 restore 切回原焦点（调用方无感）。
#[cfg(windows)]
pub fn capture_game_mcp(
    project_root: &Path,
    ratio: f64,
    crop: Option<(f64, f64, f64, f64)>,
    max_width: Option<u32>,
    player: Option<i64>,
    restore: bool,
    wait_ms: u64,
) -> Result<Value> {
    do_capture(project_root, ratio, None, false, crop, max_width, player, restore, wait_ms)
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn do_capture(
    project_root: &Path,
    ratio: f64,
    out: Option<&Path>,
    open_explorer: bool,
    crop: Option<(f64, f64, f64, f64)>,
    max_width: Option<u32>,
    player: Option<i64>,
    restore: bool,
    wait_ms: u64,
) -> Result<Value> {
    let target = locate::locate(project_root).map_err(|e| anyhow!(e))?;
    let engine_root = target.engine_root().map_err(|e| anyhow!(e))?;
    let port = bridge_client::online_port(&engine_root)
        .ok_or_else(|| anyhow!("编辑器不在线（MCP 桥不可达）。请先 editor_start 启动编辑器"))?;

    // 0. 多人定向（0.8.7）：player 给定 → mp_switch 切焦；未给定但多人局 → 缺省按玩家 1 + hint。
    //    单人局 mp_switch 回退 switched=false + note 告知（不报错不打扰）。
    let mut notes: Vec<String> = Vec::new();
    let mut restore_to: Option<i64> = None;
    let mut effective_player = player;
    if effective_player.is_none() {
        if let Ok(st) = bridge_client::bridge_rpc(port, "get_status", json!({}), 10_000) {
            if st["clients"].as_array().map(|a| a.len() > 1).unwrap_or(false) {
                effective_player = Some(1);
                notes.push("多人局未指定 player，已按玩家 1 截取".to_string());
            }
        }
    }
    if let Some(p) = effective_player {
        let sw = bridge_client::bridge_rpc(port, "mp_switch", json!({ "player": p }), 10_000)?;
        if let Some(n) = sw["note"].as_str().filter(|s| !s.is_empty()) {
            notes.push(n.to_string());
        }
        if sw["paused"].as_bool() == Some(true) {
            notes.push(format!("玩家 {p} 已暂停，画面为定格最后一帧"));
        }
        if sw["switched"].as_bool() == Some(true) {
            restore_to = sw["previous"].as_i64().filter(|prev| *prev != p);
            // 等帧：实测切焦后 45ms 即合成完成（multi-player-debug.md §4.2），缺省 200ms 余量充足
            std::thread::sleep(Duration::from_millis(wait_ms));
        }
    }
    // 焦点还原守护：无论成败截完切回原 tab（restore=false 时跳过，批量连截省切换）
    struct FocusGuard {
        port: u16,
        restore_to: Option<i64>,
    }
    impl Drop for FocusGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.restore_to.take() {
                let _ = bridge_client::bridge_rpc(self.port, "mp_switch", json!({ "player": prev }), 10_000);
            }
        }
    }
    let _focus_guard = FocusGuard {
        port,
        restore_to: if restore { restore_to } else { None },
    };

    // 1. lua 桥取 PIE 视口逻辑矩形 + 逻辑分辨率（多人局定向到目标玩家槽位）
    let rect_args = match effective_player {
        Some(p) => json!({ "player": p }),
        None => json!({}),
    };
    let rect = bridge_client::bridge_invoke(port, "lua.get_game_view_rect", rect_args, 15_000)?;
    if let Some(n) = rect["note"].as_str().filter(|s| !s.is_empty()) {
        if !notes.iter().any(|x| x == n) {
            notes.push(n.to_string());
        }
    }
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
        .or_else(|| {
            super::editor::find_editor_pid(
                &engine_root.join(super::editor::editor_exe_name(&engine_root)),
            )
        })
        .ok_or_else(|| anyhow!("找不到编辑器进程"))?;
    let main = find_window_by_class(pid, "WinUIDesktopWin32WindowClass", Some("bgd_mcp_bridge"))
        .ok_or_else(|| anyhow!("找不到编辑器主窗口（pid={pid}）"))?;
    let _restore = WindowRestoreGuard::ensure_visible(main.hwnd);

    // 3. WGC 截主窗口 + 帧内裁剪 + 倍率重采样
    let path = match out {
        Some(p) => p.to_path_buf(),
        None => {
            // 毫秒级时间戳：多步场景脚本内同一秒可能多次截图，秒级文件名会互相覆盖（0.8.0 验收实测）
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
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

    // MCP 层 crop：游戏视口逻辑坐标（与 find_ui/click_at 同系，游戏 VM 逻辑分辨率空间），
    // 先换算到编辑器 UI 逻辑坐标（视口 rect 所在空间），再 clamp 到视口范围内（越界不报错）。
    let rect_logical = match crop {
        Some((x, y, w, h)) => {
            let gi_args = match effective_player {
                Some(p) => json!({ "player": p }),
                None => json!({}),
            };
            let gi = bridge_client::bridge_invoke(port, "lua.game_info", gi_args, 10_000)?;
            let gw = gi["logical_width"].as_f64().unwrap_or(0.0);
            let gh = gi["logical_height"].as_f64().unwrap_or(0.0);
            if gw < 1.0 || gh < 1.0 {
                return Err(anyhow!(
                    "游戏侧逻辑分辨率不可用（{gi}）。游戏项目需更新框架（dbg_bus）并重新构建后 restart_last_debug"
                ));
            }
            // 越界保护：起点超出游戏画面时给出可读错误（坐标系是游戏逻辑分辨率空间，
            // 窗口尺寸变化后旧坐标会失效——提示重新 find_ui 取当前 rect）
            if x >= gw || y >= gh {
                return Err(anyhow!(
                    "crop 起点 ({x},{y}) 超出游戏画面（当前游戏逻辑分辨率 {gw}x{gh}）。\
                     坐标可能已过期（窗口尺寸变化会改变逻辑分辨率），请用 lua.find_ui 重新定位"
                ));
            }
            let sx = rw / gw;
            let sy = rh / gh;
            let ex = x * sx;
            let ey = y * sy;
            let ew = w * sx;
            let eh = h * sy;
            let cx = ex.clamp(0.0, rw);
            let cy = ey.clamp(0.0, rh);
            let cw = ew.clamp(1.0, (rw - cx).max(1.0));
            let ch = eh.clamp(1.0, (rh - cy).max(1.0));
            (rx + cx, ry + cy, cw, ch)
        }
        None => (rx, ry, rw, rh),
    };
    let map = ViewportMap {
        rect_logical,
        logical_res: (lw, lh),
        ratio,
        max_width,
    };
    let (cw, ch, nat_w, nat_h) = wgc_capture_mapped(main.hwnd, &path, &map)?;

    if open_explorer {
        let p = path.display().to_string().replace('/', "\\");
        let _ = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{p}"))
            .spawn();
    }

    // 默认值可见性纪律：max_width 默认上限会静默降采样，必须显式回显
    // 原尺寸与是否被缩放——否则 AI 拿缩放图误判像素级细节（缝隙/对齐）
    let downscaled = nat_w > cw;
    let mut ret = json!({
        "path": super::editor::to_slash(&path),
        "width": cw,
        "height": ch,
        "natural_width": nat_w,
        "natural_height": nat_h,
        "downscaled": downscaled,
        "ratio": ratio,
        "mode": "game_viewport",
    });
    if let Some(p) = effective_player {
        ret["player"] = json!(p);
    }
    if !notes.is_empty() {
        ret["note"] = json!(notes.join("；"));
    }
    if downscaled {
        let prev = ret.get("note").and_then(|v| v.as_str()).unwrap_or("");
        ret["note"] = json!(format!(
            "{prev}{}图片已被 max_width 降采样（{}x{} → {}x{}）：像素级判读（缝隙/对齐/字体）请显式调大 max_width 重截",
            if prev.is_empty() { "" } else { "；" },
            nat_w, nat_h, cw, ch
        ));
    }
    if let Some((x, y, w, h)) = crop {
        ret["crop"] = json!({ "x": x, "y": y, "w": w, "h": h });
    }
    if let Some(mw) = max_width {
        ret["max_width"] = json!(mw);
    }
    Ok(ret)
}

#[cfg(not(windows))]
pub fn capture_game_mcp(
    _project_root: &Path,
    _ratio: f64,
    _crop: Option<(f64, f64, f64, f64)>,
    _max_width: Option<u32>,
    _player: Option<i64>,
    _restore: bool,
    _wait_ms: u64,
) -> Result<Value> {
    Err(anyhow!("仅支持 Windows"))
}

/// 视口映射参数（引擎逻辑坐标 + 倍率）
#[cfg(windows)]
#[derive(Clone)]
struct ViewportMap {
    rect_logical: (f64, f64, f64, f64),
    logical_res: (f64, f64),
    ratio: f64,
    /// MCP 层输出宽度上限（最终上限，与 ratio 叠加时优先生效）
    max_width: Option<u32>,
}

/// 编辑器窗口信息
#[cfg(windows)]
struct WindowInfo {
    hwnd: *mut std::ffi::c_void,
}

/// 真后台截取守护：窗口被遮挡时 WGC 直接可截（重定向表面仍在）；窗口被最小化/隐藏时
/// GraphicsCaptureItem 创建会失败——此时把窗口**离屏显示**（SHOWNOACTIVATE + 挪到 -32000
/// 屏外坐标，用户屏幕无感知、不抢焦点），截完用保存的 WINDOWPLACEMENT 精确还原
/// （回到最小化/隐藏与原始位置）。
#[cfg(windows)]
struct WindowRestoreGuard {
    hwnd: *mut std::ffi::c_void,
    /// 原 WINDOWPLACEMENT（Drop 时还原）；None = 原本就可见无需处理
    placement: Option<windows_sys::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT>,
}

#[cfg(windows)]
impl WindowRestoreGuard {
    fn ensure_visible(hwnd: *mut std::ffi::c_void) -> Self {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowPlacement, IsIconic, IsWindowVisible, SetWindowPos, ShowWindow,
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNOACTIVATE, WINDOWPLACEMENT,
        };
        unsafe {
            let visible = IsWindowVisible(hwnd as HWND) != 0;
            let iconic = IsIconic(hwnd as HWND) != 0;
            if visible && !iconic {
                return Self { hwnd, placement: None };
            }
            // 保存原始 placement（含最小化状态与还原位置）
            let mut wp: WINDOWPLACEMENT = std::mem::zeroed();
            wp.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
            if GetWindowPlacement(hwnd as HWND, &mut wp) == 0 {
                return Self { hwnd, placement: None };
            }
            // 离屏显示（不激活、不改大小、不改变 Z 序）
            ShowWindow(hwnd as HWND, SW_SHOWNOACTIVATE);
            SetWindowPos(hwnd as HWND, std::ptr::null_mut(), -32000, -32000, 0, 0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
            // 实测：恢复后引擎重排版+首帧呈现需要 ~2s，等太短会截到黑图
            std::thread::sleep(Duration::from_millis(2500));
            Self { hwnd, placement: Some(wp) }
        }
    }
}

#[cfg(windows)]
impl Drop for WindowRestoreGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPlacement;
        if let Some(wp) = &self.placement {
            unsafe {
                SetWindowPlacement(self.hwnd as HWND, wp);
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
        IsWindowVisible,
    };

    struct Ctx {
        pid: u32,
        class: String,
        skip_title: String,
        best_hwnd: HWND,
        best_area: i64,
        best_visible_hwnd: HWND,
        best_visible_area: i64,
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
            // 可见窗口单独记录（最小化停靠的 160x28 占位窗口面积小，正常可见时主窗口最大）
            if IsWindowVisible(hwnd) != 0 && area > ctx.best_visible_area {
                ctx.best_visible_area = area;
                ctx.best_visible_hwnd = hwnd;
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
        best_visible_hwnd: std::ptr::null_mut(),
        best_visible_area: 0,
    };
    unsafe {
        EnumWindows(Some(enum_proc), &mut ctx as *mut Ctx as LPARAM);
    }
    // 优先可见的最大窗口（最小化停靠占位/隐藏辅助窗口靠后）
    let chosen = if !ctx.best_visible_hwnd.is_null() {
        ctx.best_visible_hwnd
    } else {
        ctx.best_hwnd
    };
    if chosen.is_null() {
        return None;
    }
    Some(WindowInfo { hwnd: chosen })
}

/// WGC 截窗口 + 帧内换算裁剪 + 倍率重采样。返回 (输出宽, 输出高, max_width 缩放前宽, 缩放前高)。
#[cfg(windows)]
fn wgc_capture_mapped(
    hwnd: *mut std::ffi::c_void,
    path: &Path,
    map: &ViewportMap,
) -> Result<(u32, u32, u32, u32)> {
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
    // max_width 最终上限（MCP 上下文防爆；与 ratio 同时给出时 max_width 为最终上限）
    // nat_* = 缩放前尺寸：调用方据此回显 downscaled，防 AI 拿缩放图误判像素级细节
    let (nat_w, nat_h) = (ow, oh);
    let (out_img, ow, oh) = match map.max_width {
        Some(mw) if mw >= 1 && ow > mw => {
            let nh = ((oh as f64 * mw as f64 / ow as f64).round() as u32).max(1);
            (
                image::imageops::resize(&out_img, mw, nh, image::imageops::FilterType::Lanczos3),
                mw,
                nh,
            )
        }
        _ => (out_img, ow, oh),
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
    Ok((ow, oh, nat_w, nat_h))
}
