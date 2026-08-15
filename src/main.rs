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

mod core;

use clap::Parser;
use core::{kernel, locate, log, modules, EditorTarget};
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
}

fn main() -> eframe::Result<()> {
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
            .with_min_inner_size([660.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            setup_chinese_font(&cc.egui_ctx);
            Ok(Box::new(EditorPatchApp::new(project_path)))
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
                self.target = Some(target);
                self.log("INFO", &format!("已加载项目: {}", root.display()));
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
        let task_running = self.poll_task(ctx);

        // 顶部：项目栏 + 选项卡
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("项目：");
                match &self.project_root {
                    Some(p) => {
                        ui.monospace(p.display().to_string());
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
            ui.monospace(target.editor_root.display().to_string());
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
                if !s.has_slots && s.status != kernel::LibStatus::Applied {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xc0, 0x90, 0x30),
                        "（无此版本插槽文件，将跳过）",
                    );
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
                ui.label(format!("备份目录：{}", data_dir.join("backup").display()));
                if ui.button("打开").clicked() {
                    let _ = std::process::Command::new("explorer")
                        .arg(data_dir.join("backup"))
                        .spawn();
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(format!("日志文件：{}", log_path.display()));
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
                            .map(|ids| ids.iter().any(|id| id == m.id))
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
                            ui.label(m.description);
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
            let result = lib
                .require_root_dir(target)
                .and_then(|root| modules::set_module(&root, m, on));
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
