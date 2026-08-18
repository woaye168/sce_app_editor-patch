//! 实验工具：对指定 hwnd 尝试 WGC 窗口截取，验证不同窗口类的可捕获性。
//! 用法：cargo run --example wgc_probe -- <hwnd十进制> <输出png>

use sce_app_editor_patch::core::capture_probe;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("用法: wgc_probe <hwnd> <输出png>");
        std::process::exit(1);
    }
    let hwnd: isize = args[1].parse().expect("hwnd 需为十进制整数");
    match capture_probe::probe_capture_window(hwnd as *mut std::ffi::c_void, &args[2]) {
        Ok((w, h)) => println!("OK {}x{} -> {}", w, h, args[2]),
        Err(e) => {
            eprintln!("FAIL: {e}");
            std::process::exit(1);
        }
    }
}
