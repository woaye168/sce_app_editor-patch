//! 实验：指定 hwnd 的 WGC 整窗截取探针（examples/wgc_probe 用）。

use anyhow::{anyhow, Result};
use std::time::Duration;

/// 对任意 hwnd 尝试 WGC 窗口截取（整帧保存 png），返回 (宽, 高)
#[cfg(windows)]
pub fn probe_capture_window(hwnd: *mut std::ffi::c_void, path: &str) -> Result<(u32, u32)> {
    use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
    use windows_capture::frame::{Frame, ImageFormat};
    use windows_capture::graphics_capture_api::InternalCaptureControl;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    struct CapFlags {
        path: String,
        done: std::sync::mpsc::Sender<Result<(u32, u32), String>>,
    }
    struct CapHandler {
        path: String,
        done: std::sync::mpsc::Sender<Result<(u32, u32), String>>,
    }
    impl GraphicsCaptureApiHandler for CapHandler {
        type Flags = CapFlags;
        type Error = anyhow::Error;

        fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let f = ctx.flags;
            Ok(Self { path: f.path, done: f.done })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            let r = (|| {
                let mut buf = frame.buffer().map_err(|e| format!("buffer: {e}"))?;
                let (w, h) = (buf.width(), buf.height());
                buf.save_as_image(&self.path, ImageFormat::Png)
                    .map_err(|e| format!("save: {e}"))?;
                Ok((w, h))
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
        CapFlags { path: path.to_string(), done: tx },
    );
    CapHandler::start(settings).map_err(|e| anyhow!("start: {e}"))?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| anyhow!("超时（10s 无帧，窗口可能最小化）"))?
        .map_err(|e| anyhow!(e))
}

#[cfg(not(windows))]
pub fn probe_capture_window(_hwnd: *mut std::ffi::c_void, _path: &str) -> Result<(u32, u32)> {
    Err(anyhow!("仅支持 Windows"))
}
