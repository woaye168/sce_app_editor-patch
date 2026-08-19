//! 编辑器补丁（sce_app_editor-patch）：独立应用
//!
//! 给星火编辑器打补丁：整库解密为裸露源码、入口插槽注入补丁框架、
//! 解除编辑器使用限制（isolation.lua 解锁），支持按库分组、可勾选启停的补丁模块。
//!
//! 用法：
//!   sce_app_editor-patch                      # 独立运行：启动后选择项目目录
//!   sce_app_editor-patch --project-path <DIR> # 宿主启动：直接定位项目

// Windows 下不弹出黑色控制台窗口
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;
use sce_app_editor_patch::core::{bridge_deploy, editor, kernel, locate, log, modules, EditorTarget};

/// windows 子系统下 CLI 输出会被吞；命中 CLI 时附加到父进程控制台
#[cfg(windows)]
fn attach_parent_console() {
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(0xFFFFFFFF);
    }
}
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_NAME: &str = "编辑器补丁";

#[derive(Parser)]
#[command(name = "sce_app_editor-patch", about = "给星火编辑器打补丁，实现功能扩展")]
struct Args {
    /// 项目路径（星火编辑器项目根，含 project/map_settings.json）
    #[arg(long)]
    project_path: Option<String>,
    /// 静默自启形态：不显示主窗口，驻留后台（宿主静默自启时透传）
    #[arg(long)]
    background: bool,
}

/// 应用级单实例（0.5.6）：重复启动只唤起已运行实例的窗口。
/// 机制：命名互斥体判活 + 命名事件作「显示窗口」信号；CLI 子命令（mcp/editor/logs/capture）
/// 是独立短进程，不受单实例限制。
#[cfg(windows)]
mod single_instance {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject};

    pub struct Guard {
        pub show_event: HANDLE,
        pub quit_event: HANDLE,
        _mutex: HANDLE,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.show_event);
                CloseHandle(self.quit_event);
                CloseHandle(self._mutex);
            }
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 获取单实例守卫；已存在实例时发送「显示窗口」信号并返回 None（调用方应退出）
    pub fn acquire() -> Option<Guard> {
        unsafe {
            let name = wide("sce_app_editor-patch_single");
            let mutex = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
            if mutex.is_null() {
                return None;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let ev_name = wide("sce_app_editor-patch_show");
                let ev = CreateEventW(std::ptr::null(), 0, 0, ev_name.as_ptr());
                if !ev.is_null() {
                    SetEvent(ev);
                    CloseHandle(ev);
                }
                CloseHandle(mutex);
                return None;
            }
            let ev_name = wide("sce_app_editor-patch_show");
            let show_ev = CreateEventW(std::ptr::null(), 0, 0, ev_name.as_ptr());
            let quit_name = wide("sce_app_editor-patch_quit");
            let quit_ev = CreateEventW(std::ptr::null(), 0, 0, quit_name.as_ptr());
            Some(Guard { show_event: show_ev, quit_event: quit_ev, _mutex: mutex })
        }
    }

    /// 「显示窗口」信号是否已触发
    pub fn show_signaled(show_event: HANDLE) -> bool {
        unsafe { WaitForSingleObject(show_event, 0) == 0 }
    }

    /// 「退出」信号是否已触发
    pub fn quit_signaled(quit_event: HANDLE) -> bool {
        unsafe { WaitForSingleObject(quit_event, 0) == 0 }
    }

    /// 向已运行实例发送「退出」信号（宿主升级前优雅停止用）
    pub fn signal_quit() {
        unsafe {
            let quit_name = wide("sce_app_editor-patch_quit");
            let ev = CreateEventW(std::ptr::null(), 0, 0, quit_name.as_ptr());
            if !ev.is_null() {
                SetEvent(ev);
                CloseHandle(ev);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    // CLI 子命令（0.5.4 起：编辑器控制能力随应用自持）：editor/logs/capture/mcp
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = raw.first().map(|s| s.as_str()) {
        match first {
            "mcp" => {
                // stdio MCP 走管道，不需要附加控制台
                std::process::exit(sce_app_editor_patch::mcp::run_stdio());
            }
            "editor" | "logs" | "capture" => {
                #[cfg(windows)]
                attach_parent_console();
                std::process::exit(sce_app_editor_patch::cli::run(&raw));
            }
            _ => {}
        }
    }

    // --quit：向已运行实例发「退出」信号后退出（宿主升级前优雅停止用，0.5.7）
    #[cfg(windows)]
    if raw.iter().any(|a| a == "--quit") {
        single_instance::signal_quit();
        return Ok(());
    }

    // GUI 路径单实例（0.5.6）：已运行则只发「唤起窗口」信号并退出（静默自启下点「打开」= 唤出窗口）
    #[cfg(windows)]
    let single_guard = match single_instance::acquire() {
        Some(g) => Some(g),
        None => return Ok(()),
    };

    let args = Args::parse();

    // 项目路径：优先 --project-path，否则启动后由用户在界面选择
    let project_path = args.project_path.map(PathBuf::from).and_then(|p| {
        if locate::is_valid_project(&p) {
            Some(p)
        } else {
            eprintln!("警告：{} 不是有效的星火项目（缺少 project/map_settings.json）", p.display());
            None
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("{APP_NAME} v{APP_VERSION}"))
            .with_inner_size([780.0, 660.0])
            .with_min_inner_size([660.0, 520.0])
            // --background（宿主静默自启）：不显示主窗口，驻留后台等唤起信号
            .with_visible(!args.background),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            setup_chinese_font(&cc.egui_ctx);
            #[allow(unused_mut)]
            let mut app = EditorPatchApp::new(project_path);
            #[cfg(windows)]
            {
                app.single_guard = single_guard;
            }
            Ok(Box::new(app))
        }),
    )
}

/// 加载系统中文字体（微软雅黑），egui 默认字体不含中文
fn setup_chinese_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for candidate in ["C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/simhei.ttf"] {
        if let Ok(data) = std::fs::read(candidate) {
            fonts
                .font_data
                .insert("chinese".to_string(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "chinese".to_string());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("chinese".to_string());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Kernel,
    Patches,
    Settings,
    Help,
}

struct EditorPatchApp {
    tab: Tab,
    /// 当前项目根
    project_root: Option<PathBuf>,
    /// 定位结果（编辑器根 + 各包目录）
    target: Option<EditorTarget>,
    /// 定位失败原因
    locate_error: String,
    /// 各库补丁状态
    statuses: Vec<kernel::LibStatusInfo>,
    /// 内置补丁模块
    modules: Vec<modules::PatchModule>,
    /// 各库已启用的模块 id（pkg → ids）
    enabled: BTreeMap<String, Vec<String>>,
    /// 正在执行的后台任务（应用/还原）
    task: Option<kernel::SharedProgress>,
    status: String,
    /// MCP 端口配置（文本框，默认 39177）
    mcp_port_input: String,
    /// 编辑器 exe 名配置（文本框，默认 星火编辑器.exe）
    exe_name_input: String,
    /// 单实例守卫（GUI 驻留期间持有；轮询「唤起窗口」信号）
    #[cfg(windows)]
    single_guard: Option<single_instance::Guard>,
}

impl EditorPatchApp {
    fn new(project_root: Option<PathBuf>) -> Self {
        let mut app = Self {
            tab: Tab::Kernel,
            project_root: None,
            target: None,
            locate_error: String::new(),
            statuses: Vec::new(),
            modules: modules::builtin_modules(),
            enabled: BTreeMap::new(),
            task: None,
            status: String::new(),
            mcp_port_input: String::new(),
            exe_name_input: String::new(),
            #[cfg(windows)]
            single_guard: None,
        };
        if let Some(root) = project_root {
            app.set_project(root);
        } else {
            app.status = "请先选择星火编辑器项目目录".to_string();
        }
        app
    }

    fn project_root(&self) -> Option<&std::path::Path> {
        self.project_root.as_deref()
    }

    fn editor_root(&self) -> Option<&std::path::Path> {
        self.target.as_ref().map(|t| t.editor_root.as_path())
    }

    /// 指定库是否已应用：决定该库的补丁模块是否可操作
    fn lib_applied(&self, pkg: &str) -> bool {
        self.statuses
            .iter()
            .any(|s| s.pkg == pkg && s.status == kernel::LibStatus::Applied)
    }

    fn log(&self, level: &str, msg: &str) {
        log::log(self.project_root(), self.editor_root(), level, msg);
    }

    fn set_project(&mut self, root: PathBuf) {
        editor::set_last_project_path(&root);
        self.project_root = Some(root);
        self.refresh();
    }

    /// 重新定位 + 检查全部库状态 + 扫描各库已启用模块
    fn refresh(&mut self) {
        let Some(root) = self.project_root.clone() else {
            return;
        };
        match locate::locate(&root) {
            Ok(target) => {
                self.statuses = kernel::check(&target);
                self.enabled.clear();
                for lib in kernel::LIBS {
                    if let Ok(dir) = lib.require_root_dir(&target) {
                        self.enabled
                            .insert(lib.pkg.to_string(), modules::enabled_modules(&dir));
                    }
                }
                self.locate_error.clear();
                self.status = format!("已定位：编辑器版本 {}", target.api_version);
                self.exe_name_input = editor::editor_exe_name(&target.engine_root());
                self.target = Some(target);
                self.log("INFO", &format!("已加载项目: {}", root.display()));
                self.load_mcp_port();
                self.sync_injected_project_roots();
                self.auto_redeploy_bridge();
            }
            Err(e) => {
                self.target = None;
                self.enabled.clear();
                self.statuses.clear();
                self.locate_error = e.clone();
                self.status = format!("定位失败: {e}");
                log::log(Some(&root), None, "ERROR", &format!("定位失败: {e}"));
            }
        }
    }

    /// 已启用模块的自同步（refresh 时执行）：
    /// - F1 项目切换后刷新 inject_project_root 模块注入的项目路径；
    /// - 0.5.7 应用升级后按嵌 exe 内容刷新模块部署文件（sync_module_files）；
    /// - inject_exe_path 模块的 exe 路径刷新。
    fn sync_injected_project_roots(&mut self) {
        let Some(root) = self.project_root.clone() else { return };
        let Some(target) = &self.target else { return };
        for m in &self.modules {
            let enabled = self
                .enabled
                .get(m.pkg.as_str())
                .map(|ids| ids.iter().any(|i| i == &m.id))
                .unwrap_or(false);
            if !enabled {
                continue;
            }
            let Some(lib) = kernel::LIBS.iter().find(|l| l.pkg == m.pkg) else {
                continue;
            };
            let Ok(lib_root) = lib.require_root_dir(target) else {
                continue;
            };
            match modules::sync_project_root(&lib_root, m, &root) {
                Ok(true) => {
                    self.log(
                        "INFO",
                        &format!("模块[{}]注入的项目路径已自动更新为: {}", m.id, root.display()),
                    );
                    self.status = format!(
                        "检测到项目切换，已自动更新模块「{}」注入的项目路径（重启星火编辑器后生效）",
                        m.name
                    );
                }
                Ok(false) => {}
                Err(e) => {
                    self.log("ERROR", &format!("模块[{}]项目路径同步失败: {e}", m.id));
                }
            }
            // 0.5.7：应用升级后自动更新已启用模块的部署文件（嵌 exe 内容为准）
            match modules::sync_module_files(&lib_root, m) {
                Ok(true) => {
                    self.log("INFO", &format!("模块[{}]部署文件已自动更新（重启星火编辑器后生效）", m.id));
                    self.status = format!(
                        "模块「{}」已随应用升级自动更新（重启星火编辑器后生效）",
                        m.name
                    );
                }
                Ok(false) => {}
                Err(e) => {
                    self.log("ERROR", &format!("模块[{}]部署文件同步失败: {e}", m.id));
                }
            }
            match modules::sync_exe_path(&lib_root, m) {
                Ok(true) => {
                    self.log("INFO", &format!("模块[{}]注入的 exe 路径已自动更新", m.id));
                }
                Ok(false) => {}
                Err(e) => {
                    self.log("ERROR", &format!("模块[{}]exe 路径同步失败: {e}", m.id));
                }
            }
        }
    }

    /// 模块勾选状态保留但内嵌 dll 已更新时，自动重新部署 bgd_mcp_bridge.dll。
    /// 场景：应用市场升级本应用后重启——补丁目录里模块仍是勾选态，但 exe 内嵌的 dll 已是新版。
    /// 此时若 version 目录里的 dll 与内嵌不一致，自动重写（编辑器开着会锁定 dll，失败仅记日志提示）。
    fn auto_redeploy_bridge(&mut self) {
        let Some(target) = &self.target else { return };
        // 仅当 bgd_mcp_bridge 处于勾选状态才考虑重部署
        let enabled_here = self
            .enabled
            .values()
            .any(|ids| ids.iter().any(|i| i == "bgd_mcp_bridge"));
        if !enabled_here {
            return;
        }
        let vdir = target.version_dir();
        if !bridge_deploy::needs_redeploy(&vdir) {
            return;
        }
        match bridge_deploy::deploy(&vdir) {
            Ok(()) => {
                self.log("INFO", "检测到 bgd_mcp_bridge.dll 版本过旧，已自动更新（重启编辑器后生效）");
                self.status = "已自动更新 bgd_mcp_bridge.dll（重启编辑器后生效）".to_string();
            }
            Err(e) => {
                self.log("ERROR", &format!("自动更新 bgd_mcp_bridge.dll 失败: {e}"));
                self.status = format!("bgd_mcp_bridge.dll 需更新但写入失败（编辑器可能正在运行）：{e}");
            }
        }
    }

    /// MCP 配置文件路径：<引擎运行根>/logs/bgd_csharp/config.json
    fn mcp_config_path(&self) -> Option<PathBuf> {
        self.target
            .as_ref()
            .map(|t| t.engine_root().join("logs").join("bgd_csharp").join("config.json"))
    }

    /// 从 config.json 读端口到输入框（无配置则用默认 39177）
    fn load_mcp_port(&mut self) {
        let port = self
            .mcp_config_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("mcp_port").and_then(|x| x.as_u64()))
            .map(|p| p.to_string())
            .unwrap_or_else(|| "39177".to_string());
        self.mcp_port_input = port;
    }

    /// 保存端口到 config.json（C# 侧启动时读取，重启编辑器后生效）
    fn save_mcp_port(&mut self) {
        let trimmed = self.mcp_port_input.trim();
        let port: u64 = match trimmed.parse() {
            Ok(p) if (1025..65535).contains(&p) => p,
            _ => {
                self.status = format!("端口无效（需 1025-65534 的整数）：{trimmed}");
                return;
            }
        };
        let Some(path) = self.mcp_config_path() else {
            self.status = "未定位到编辑器，无法保存端口配置".to_string();
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.status = format!("创建配置目录失败: {e}");
                return;
            }
        }
        let body = serde_json::json!({ "mcp_port": port }).to_string();
        match std::fs::write(&path, body) {
            Ok(()) => {
                self.status = format!("MCP 端口已保存为 {port}（重启星火编辑器后生效）");
                self.log("INFO", &format!("MCP 端口已保存: {port}"));
            }
            Err(e) => {
                self.status = format!("保存端口配置失败: {e}");
            }
        }
    }

    fn pick_project(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("选择星火编辑器项目目录")
            .pick_folder()
        {
            if locate::is_valid_project(&dir) {
                self.set_project(dir);
            } else {
                self.status =
                    format!("{} 不是有效的星火项目（缺少 project/map_settings.json）", dir.display());
            }
        }
    }

    /// 启动后台任务（应用/还原）
    fn start_task(&mut self, restore: bool) {
        if self.task.is_some() {
            return;
        }
        let Some(root) = self.project_root.clone() else {
            return;
        };
        self.log("INFO", if restore { "用户操作：还原补丁" } else { "用户操作：应用补丁" });
        self.task = Some(if restore {
            kernel::restore_async(root)
        } else {
            kernel::apply_async(root)
        });
    }

    /// 轮询后台任务进度（每帧调用），返回是否正在执行
    fn poll_task(&mut self, ctx: &egui::Context) -> bool {
        let Some(task) = &self.task else {
            return false;
        };
        let (finished, ok, summary) = {
            let g = task.lock().unwrap();
            (g.finished, g.ok, g.summary.clone())
        };
        if finished {
            self.status = format!(
                "{}（重启星火编辑器后生效）：\n{}",
                if ok { "成功" } else { "失败/未完成" },
                summary
            );
            self.task = None;
            self.refresh();
            return false;
        }
        // 任务执行中：定时刷新进度条
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        true
    }
}

impl eframe::App for EditorPatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 单实例唤起/退出信号
        #[cfg(windows)]
        if let Some(g) = &self.single_guard {
            if single_instance::show_signaled(g.show_event) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.log("INFO", "检测到重复启动，已唤起窗口");
            }
            if single_instance::quit_signaled(g.quit_event) {
                self.log("INFO", "收到退出信号（宿主升级/停止请求），应用退出");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        // 隐藏驻留时 egui 无事件不触发 update——周期唤醒保证信号轮询（2 次/秒，开销可忽略）
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        let task_running = self.poll_task(ctx);

        // 顶部：项目栏 + 选项卡
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("项目：");
                match &self.project_root {
                    Some(p) => {
                        ui.monospace(editor::to_slash(p));
                    }
                    None => {
                        ui.label("（未选择）");
                    }
                }
                if ui.button("选择项目…").clicked() {
                    self.pick_project();
                }
            });
            ui.add_space(4.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Kernel, "内核");
                ui.selectable_value(&mut self.tab, Tab::Patches, "补丁");
                ui.selectable_value(&mut self.tab, Tab::Settings, "设置");
                ui.selectable_value(&mut self.tab, Tab::Help, "帮助");
            });
        });

        // 底部：状态栏
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(&self.status);
            ui.add_space(2.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // 任务执行中：顶部进度条
            if task_running {
                if let Some(task) = &self.task {
                    let g = task.lock().unwrap();
                    let done = g.done.load(Ordering::Relaxed);
                    let total = g.total.max(1);
                    ui.add_space(4.0);
                    ui.add(
                        egui::ProgressBar::new(done as f32 / total as f32)
                            .text(format!("{}（{}/{}）", g.phase, done, g.total)),
                    );
                    ui.add_space(4.0);
                    ui.separator();
                }
            }
            match self.tab {
                Tab::Kernel => self.ui_kernel(ui, task_running),
                Tab::Patches => self.ui_patches(ui),
                Tab::Settings => self.ui_settings(ui),
                Tab::Help => Self::ui_help(ui),
            }
        });
    }
}

impl EditorPatchApp {
    /// 无项目时的占位界面，返回是否已显示占位
    fn ui_need_project(&mut self, ui: &mut egui::Ui) -> bool {
        if self.project_root.is_some() {
            return false;
        }
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("请先选择星火编辑器项目目录");
            if ui.button("选择项目…").clicked() {
                self.pick_project();
            }
        });
        true
    }

    fn ui_kernel(&mut self, ui: &mut egui::Ui, task_running: bool) {
        // MCP 配置复制（不依赖项目定位，置顶常显）
        ui.horizontal(|ui| {
            ui.label("AI 客户端 MCP 入口：");
            if ui.button("复制 MCP 配置").clicked() {
                let exe = std::env::current_exe()
                    .map(|p| p.display().to_string().replace('\\', "/"))
                    .unwrap_or_else(|_| "sce_app_editor-patch.exe".to_string());
                let json = format!(
                    "{{ \"mcpServers\": {{ \"bgd-sce\": {{ \"command\": \"{exe}\", \"args\": [\"mcp\"] }} }} }}"
                );
                ui.ctx().output_mut(|o| o.copied_text = json);
                self.status = "MCP 配置已复制到剪贴板（粘贴到 AI 客户端的 MCP 设置即可）".to_string();
            }
        });
        ui.add_space(2.0);
        ui.weak("AGENT 只需配置这一个 stdio MCP（编辑器控制/调试/日志/截图/发布全链路）。");
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        if self.ui_need_project(ui) {
            return;
        }
        ui.heading("内核补丁");
        ui.add_space(8.0);

        if !self.locate_error.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(0xd0, 0x50, 0x50), &self.locate_error);
            if ui.button("重新定位").clicked() {
                self.refresh();
            }
            return;
        }

        let Some(target) = &self.target else {
            return;
        };

        // 定位信息
        ui.horizontal(|ui| {
            ui.label("编辑器版本：");
            ui.monospace(&target.api_version);
            ui.separator();
            ui.label("编辑器目录：");
            ui.monospace(editor::to_slash(&target.editor_root));
        });
        ui.add_space(8.0);

        // 各库补丁状态
        ui.label("库补丁状态：");
        ui.add_space(4.0);
        let mut any_not_applied = false;
        let mut any_applied = false;
        for s in &self.statuses {
            ui.horizontal(|ui| {
                match s.status {
                    kernel::LibStatus::Applied => {
                        any_applied = true;
                        ui.colored_label(egui::Color32::from_rgb(0x3a, 0xa0, 0x50), "● 已应用");
                    }
                    kernel::LibStatus::NotApplied => {
                        any_not_applied = true;
                        ui.colored_label(egui::Color32::from_rgb(0xc0, 0x90, 0x30), "○ 未应用");
                    }
                    kernel::LibStatus::Missing => {
                        ui.colored_label(egui::Color32::from_rgb(0xd0, 0x50, 0x50), "✘ 缺失");
                    }
                }
                ui.label(format!("{} [v{}]", s.label, s.version)).on_hover_text(&s.path);
                let hint = s.slot_level.hint();
                if !hint.is_empty() && s.status != kernel::LibStatus::Applied {
                    ui.colored_label(egui::Color32::from_rgb(0xc0, 0x90, 0x30), hint);
                }
                if s.has_backup {
                    ui.monospace("（有备份）");
                }
            });
        }
        // 覆盖提示：部分应用 = 可能被编辑器升级覆盖
        if any_applied && any_not_applied {
            ui.add_space(4.0);
            ui.colored_label(
                egui::Color32::from_rgb(0xc0, 0x90, 0x30),
                "检测到部分库未应用：可能被编辑器升级覆盖，点击「应用补丁」重新应用即可。",
            );
        }
        ui.add_space(12.0);

        // 操作（任务执行中禁用）
        ui.horizontal(|ui| {
            if ui.add_enabled(!task_running, egui::Button::new("应用补丁")).clicked() {
                self.start_task(false);
            }
            if ui.add_enabled(!task_running, egui::Button::new("还原补丁")).clicked() {
                self.start_task(true);
            }
            if ui.add_enabled(!task_running, egui::Button::new("刷新状态")).clicked() {
                self.refresh();
            }
        });
        ui.add_space(8.0);
        ui.label("「应用补丁」把目标库整库解密为裸露源码、解锁禁用函数、在库入口注入补丁插槽；");
        ui.label("「还原补丁」用整库备份原样还原，并移除全部补丁模块。");
        ui.label("补丁状态可随时「刷新状态」检测：编辑器升级覆盖补丁后重新应用即可。");

        // 备份 / 日志目录
        let log_path = log::log_path(self.project_root(), self.editor_root());
        if let Some(root) = self.editor_root() {
            let data_dir = root.join("bgd_editor_patch");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(format!("备份目录：{}", editor::to_slash(&data_dir.join("backup"))));
                if ui.button("打开").clicked() {
                    let _ = std::process::Command::new("explorer")
                        .arg(data_dir.join("backup"))
                        .spawn();
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(format!("日志文件：{}", editor::to_slash(&log_path)));
            if ui.button("打开").clicked() {
                let _ = std::process::Command::new("explorer").arg(&log_path).spawn();
            }
        });
    }

    fn ui_patches(&mut self, ui: &mut egui::Ui) {
        if self.ui_need_project(ui) {
            return;
        }
        ui.heading("补丁模块");
        ui.add_space(8.0);

        if self.target.is_none() {
            ui.label("定位失败，请先在「内核」标签页检查");
            return;
        }

        ui.label("按库分组，勾选启用补丁模块（重启星火编辑器后生效）：");
        ui.add_space(4.0);

        let mut toggled: Option<(usize, bool)> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for lib in kernel::LIBS {
                let lib_modules: Vec<(usize, &modules::PatchModule)> = self
                    .modules
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.pkg == lib.pkg)
                    .collect();
                if lib_modules.is_empty() {
                    continue;
                }
                let applied = self.lib_applied(lib.pkg);
                ui.horizontal(|ui| {
                    ui.heading(format!("{} [v{}]", lib.name, self.lib_version(lib.pkg)));
                    if !applied {
                        ui.colored_label(
                            egui::Color32::from_rgb(0xc0, 0x90, 0x30),
                            "（内核未应用，请先在「内核」标签页应用补丁）",
                        );
                    }
                });
                ui.indent(format!("lib_{}", lib.pkg), |ui| {
                    for (idx, m) in lib_modules {
                        let mut on = self
                            .enabled
                            .get(lib.pkg)
                            .map(|ids| ids.iter().any(|id| id == &m.id))
                            .unwrap_or(false);
                        ui.horizontal(|ui| {
                            let cb = egui::Checkbox::new(
                                &mut on,
                                format!("{}{}", m.name, if m.default_enabled { "（默认）" } else { "" }),
                            );
                            if ui.add_enabled(applied, cb).changed() {
                                toggled = Some((idx, on));
                            }
                            ui.monospace(format!("({})", m.id));
                        });
                        ui.indent(format!("desc_{}", m.id), |ui| {
                            ui.label(&m.description);
                        });
                        ui.add_space(2.0);
                    }
                });
                ui.add_space(6.0);
            }
        });

        if let Some((idx, on)) = toggled {
            let Some(target) = &self.target else { return };
            let m = &self.modules[idx];
            let Some(lib) = kernel::LIBS.iter().find(|l| l.pkg == m.pkg) else {
                return;
            };
            let project_root = self.project_root.clone();
            let result = lib
                .require_root_dir(target)
                .and_then(|root| {
                    // 声明了 deploy_bridge_dll 的模块需要引擎版本目录（部署/摘除 dll）
                    let version_dir = if m.deploy_bridge_dll {
                        let vdir = target.version_dir();
                        if !vdir.is_dir() {
                            return Err(format!(
                                "引擎版本目录不存在（无法部署 bgd_mcp_bridge.dll）: {}",
                                vdir.display()
                            ));
                        }
                        Some(vdir)
                    } else {
                        None
                    };
                    modules::set_module(&root, m, on, version_dir.as_deref(), project_root.as_deref())
                });
            match result {
                Ok(()) => {
                    self.status = format!(
                        "模块「{}」已{}（重启星火编辑器后生效）",
                        m.name,
                        if on { "启用" } else { "关闭" }
                    );
                    self.log(
                        "INFO",
                        &format!("模块[{}]{}", m.id, if on { "启用" } else { "关闭" }),
                    );
                }
                Err(e) => {
                    self.status = format!("操作失败: {e}");
                    self.log("ERROR", &format!("模块[{}]操作失败: {e}", m.id));
                }
            }
            self.refresh();
        }
    }

    fn lib_version(&self, pkg: &str) -> String {
        self.statuses
            .iter()
            .find(|s| s.pkg == pkg)
            .map(|s| s.version.clone())
            .unwrap_or_else(|| "?".to_string())
    }

    /// 「设置」标签页：MCP 服务端口配置
    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("设置");
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label("MCP 桥服务（bgd_mcp_bridge）");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("监听端口：");
            ui.add(egui::TextEdit::singleline(&mut self.mcp_port_input).desired_width(80.0));
            if ui.button("保存").clicked() {
                self.save_mcp_port();
            }
        });
        ui.add_space(2.0);
        ui.weak("默认 39177。端口被占用时可改为其他端口（1025-65534）。");
        ui.weak("保存后需重启星火编辑器生效；外部 AI 工具按 http://127.0.0.1:<端口>/mcp 配置。");
        ui.weak("注意避开系统保留端口段（Hyper-V/WSL 会动态保留整段，netstat 查不到占用），可用 netsh int ipv4 show excludedportrange tcp 查看。");
        ui.weak("启动时若配置端口不可用（被占/在保留段内），服务会自动向后避让并把实际端口写入 logs/bgd_csharp/port 文件。");
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label("编辑器可执行文件");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("exe 文件名：");
            ui.add(egui::TextEdit::singleline(&mut self.exe_name_input).desired_width(180.0));
            if ui.button("保存").clicked() {
                let name = self.exe_name_input.trim().to_string();
                if name.is_empty() || !name.ends_with(".exe") {
                    self.status = format!("exe 名无效（需以 .exe 结尾）：{name}");
                } else if let Some(target) = &self.target {
                    match editor::set_editor_exe_name(&target.engine_root(), &name) {
                        Ok(()) => {
                            self.status = format!("编辑器 exe 名已保存为 {name}（editor start 即时生效）");
                            self.log("INFO", &format!("编辑器 exe 名已保存: {name}"));
                        }
                        Err(e) => self.status = format!("保存失败: {e}"),
                    }
                } else {
                    self.status = "未定位到编辑器，无法保存".to_string();
                }
            }
        });
        ui.add_space(2.0);
        ui.weak("默认 星火编辑器.exe。若你的编辑器 exe 改过名，在此修改（editor start / MCP editor_start 按此名启动）。");
    }

    fn ui_help(ui: &mut egui::Ui) {
        ui.heading("帮助");
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                "【本应用的作用】\n\
                 编辑器补丁（sce_app_editor-patch）用于给星火编辑器打补丁，扩展编辑器自身能力：\n\
                 · 内核补丁（按库）：把目标库整库解密为裸露源码（方便查看与二次开发），\n\
                   在库入口注入补丁插槽；script 库额外解锁 isolation.lua 被禁用的函数。\n\
                 · 补丁模块：按库分组的可勾选功能模块（如编辑器菜单扩展、解除项目文件监听）。\n\
                 \n\
                 【使用方法】\n\
                 1. 通过 bgd_sce_tools 的「应用 - 应用市场」安装本应用并打开（会自动传入当前项目）。\n\
                 2. 在「内核」标签页点击「应用补丁」（整库解密有进度条，稍候片刻）。\n\
                 3. 在「补丁」标签页按库勾选需要的补丁模块。\n\
                 \n\
                 【重要】\n\
                 · 应用补丁 / 还原补丁 / 启停模块后，必须【重启星火编辑器】才能生效！\n\
                 · 星火编辑器升级可能覆盖补丁：打开本应用点「刷新状态」即可检测，\n\
                   显示「未应用」时点「应用补丁」重新应用即可（已启用的模块会保留）。\n\
                 \n\
                 【安全性】\n\
                 · 首次应用前对整个库做完整备份（<编辑器目录>/bgd_editor_patch/backup/，\n\
                   随编辑器数据走，本应用卸载/重装不会丢）。\n\
                 · 库内加密/明文文件混合自动识别，明文文件不会被误处理。\n\
                 · 写入采用临时文件原子替换，避免写一半损坏编辑器。\n\
                 · 操作日志记录在项目 .bgd/log/ 下，按日期分文件。",
            );
        });
    }
}
