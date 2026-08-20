//! 编辑器补丁（sce_app_editor-patch）：独立应用
//!
//! 给星火编辑器打补丁：整库解密为裸露源码、入口插槽注入补丁框架、
//! 解除编辑器使用限制（isolation.lua 解锁），支持按库分组、可勾选启停的补丁模块。
//!
//! 用法：
//!   sce_app_editor-patch                      # 独立运行：启动后选择项目目录
//!   sce_app_editor-patch --project-path <DIR> # 宿主启动：直接定位项目
//!
//! 本文件为入口聚合：CLI 分发 / 应用状态与后台任务 / ShellApp 壳实现；
//! 四个标签页 UI 分散在 ui_{kernel,patches,settings,help}.rs（impl EditorPatchApp）。

// Windows 下不弹出黑色控制台窗口
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use sce_app_editor_patch::core::{bridge_deploy, editor, kernel, locate, log, modules, EditorTarget};

// 业务 UI 按标签页拆分（impl EditorPatchApp 分散在各文件中）
mod ui_help;
mod ui_kernel;
mod ui_patches;
mod ui_settings;

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

fn main() -> eframe::Result<()> {
    // panic 落盘（GUI 是 windows 子系统，崩溃默认无任何输出）
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write(
            std::env::temp_dir().join("ep_panic.txt"),
            format!("{info}"),
        );
    }));

    // CLI 子命令（0.5.4 起：编辑器控制能力随应用自持）：editor/logs/capture/mcp/notify
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = raw.first().map(|s| s.as_str()) {
        match first {
            "mcp" => {
                // stdio MCP 走管道，不需要附加控制台
                std::process::exit(sce_app_editor_patch::mcp::run_stdio());
            }
            "editor" | "logs" | "capture" | "notify" => {
                #[cfg(windows)]
                attach_parent_console();
                std::process::exit(sce_app_editor_patch::cli::run(&raw));
            }
            _ => {}
        }
    }

    // 应用统一入口（bgd_appsdk 全托管公共逻辑：--quit/单实例/看守线程/项目解析/窗口壳）
    bgd_appsdk::app::run(
        bgd_appsdk::app::AppOptions {
            app_name: APP_NAME,
            inner_size: [780.0, 660.0],
            min_size: [660.0, 520.0],
            si_prefix: None,
            is_valid_project: Some(|p| locate::is_valid_project(p)),
            app: EditorPatchApp::new(None),
        },
        APP_VERSION,
    )
}

struct EditorPatchApp {
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
    /// bgd_mcp_bridge.dll 待重部署标志（编辑器占用导致部署失败后，update 周期重试）
    bridge_redeploy_pending: bool,
    /// 上次重部署重试时间
    last_redeploy_retry: std::time::Instant,
}

impl EditorPatchApp {
    fn new(project_root: Option<PathBuf>) -> Self {
        let mut app = Self {
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
            bridge_redeploy_pending: false,
            last_redeploy_retry: std::time::Instant::now(),
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
        let located = locate::locate(&root).and_then(|t| t.engine_root().map(|_| t));
        match located {
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
                if let Ok(engine_root) = target.engine_root() {
                    self.exe_name_input = editor::editor_exe_name(&engine_root);
                }
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
    /// - 运行时共享常量 bgd_runtime.lua 按当前项目刷新（每库一次）；
    /// - 0.5.7 应用升级后按嵌 exe 内容刷新模块部署文件（sync_module_files）；
    /// - inject_exe_path 模块的 exe 路径刷新。
    fn sync_injected_project_roots(&mut self) {
        let Some(root) = self.project_root.clone() else { return };
        let Some(target) = &self.target else { return };
        // 运行时共享常量（项目路径）：按库刷新一次（有 inject_project_root 模块启用时）
        for lib in kernel::LIBS {
            let needs = self.modules.iter().any(|m| {
                m.pkg == lib.pkg
                    && m.inject_project_root
                    && self
                        .enabled
                        .get(lib.pkg)
                        .map(|ids| ids.iter().any(|i| i == &m.id))
                        .unwrap_or(false)
            });
            if !needs {
                continue;
            }
            let Ok(lib_root) = lib.require_root_dir(target) else {
                continue;
            };
            match modules::sync_runtime_config(&lib_root, &root) {
                Ok(true) => {
                    self.log("INFO", &format!("运行时共享常量项目路径已更新为: {}", root.display()));
                    self.status = "运行时共享常量（项目路径）已更新（重启星火编辑器后生效）".to_string();
                }
                Ok(false) => {}
                Err(e) => {
                    self.log("ERROR", &format!("运行时共享常量同步失败: {e}"));
                }
            }
        }
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
        let Ok(vdir) = target.version_dir() else {
            return;
        };
        if !bridge_deploy::needs_redeploy(&vdir) {
            self.bridge_redeploy_pending = false;
            return;
        }
        match bridge_deploy::deploy(&vdir) {
            Ok(()) => {
                self.bridge_redeploy_pending = false;
                self.log("INFO", "检测到 bgd_mcp_bridge.dll 版本过旧，已自动更新（重启编辑器后生效）");
                self.status = "已自动更新 bgd_mcp_bridge.dll（重启编辑器后生效）".to_string();
            }
            Err(e) => {
                // 失败（多为编辑器运行中占用 dll）→ 置待重试标志，update 周期重试
                self.bridge_redeploy_pending = true;
                self.log("ERROR", &format!("自动更新 bgd_mcp_bridge.dll 失败: {e}"));
                self.status = format!(
                    "bgd_mcp_bridge.dll 需更新但写入失败（编辑器运行中占用），将持续自动重试：{e}"
                );
            }
        }
    }

    /// MCP 配置文件路径：<引擎运行根>/logs/bgd_csharp/config.json
    fn mcp_config_path(&self) -> Option<PathBuf> {
        self.target
            .as_ref()
            .and_then(|t| t.engine_root().ok())
            .map(|r| r.join("logs").join("bgd_csharp").join("config.json"))
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

const TABS: &[bgd_appsdk::ui::ShellTab] = &[
    bgd_appsdk::ui::ShellTab { id: "kernel", label: "内核" },
    bgd_appsdk::ui::ShellTab { id: "patches", label: "补丁" },
    bgd_appsdk::ui::ShellTab { id: "settings", label: "设置" },
    bgd_appsdk::ui::ShellTab { id: "help", label: "帮助" },
];

impl bgd_appsdk::ui::ShellApp for EditorPatchApp {
    fn app_title(&self) -> &'static str {
        APP_NAME
    }

    fn tabs(&self) -> &[bgd_appsdk::ui::ShellTab] {
        TABS
    }

    fn ui_tab(&mut self, ui: &mut egui::Ui, tab: &str) {
        // 周期任务（壳的 update 已每 500ms 唤醒）：重部署重试 + 进度条刷新
        if self.bridge_redeploy_pending
            && self.last_redeploy_retry.elapsed() >= std::time::Duration::from_secs(5)
        {
            self.last_redeploy_retry = std::time::Instant::now();
            self.auto_redeploy_bridge();
        }
        let task_running = self.poll_task(ui.ctx());

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
        match tab {
            "kernel" => self.ui_kernel(ui, task_running),
            "patches" => self.ui_patches(ui),
            "settings" => self.ui_settings(ui),
            "help" => Self::ui_help(ui),
            _ => {}
        }
    }

    fn on_project_changed(&mut self, project: Option<&std::path::Path>) {
        if let Some(p) = project {
            if locate::is_valid_project(p) {
                self.set_project(p.to_path_buf());
            } else {
                self.status = format!("{} 不是有效的星火项目（缺少 project/map_settings.json）", p.display());
            }
        }
    }

    fn status_text(&self) -> String {
        self.status.clone()
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
}
