//! 编辑器补丁（sce_app_editor-patch）：独立应用
//!
//! 给星火编辑器打补丁：解除编辑器使用限制（isolation.lua 解锁），
//! 注入补丁框架入口，支持可勾选启停的补丁模块。
//!
//! 用法：
//!   sce_app_editor-patch                      # 独立运行：启动后选择项目目录
//!   sce_app_editor-patch --project-path <DIR> # 宿主启动：直接定位项目

// Windows 下不弹出黑色控制台窗口
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod core;

use clap::Parser;
use core::{kernel, locate, modules, EditorTarget};
use std::path::PathBuf;

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
            .with_inner_size([760.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
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
    /// 定位结果（编辑器 common 包等）
    target: Option<EditorTarget>,
    /// 定位失败原因
    locate_error: String,
    /// 内核状态
    kernel_status: kernel::KernelStatus,
    /// 是否有 isolation.lua 备份
    has_backup: bool,
    /// 内置补丁模块
    modules: Vec<modules::PatchModule>,
    /// 已启用的模块 id
    enabled: Vec<String>,
    status: String,
}

impl EditorPatchApp {
    fn new(project_root: Option<PathBuf>) -> Self {
        let mut app = Self {
            tab: Tab::Kernel,
            project_root: None,
            target: None,
            locate_error: String::new(),
            kernel_status: kernel::KernelStatus::Unknown,
            has_backup: false,
            modules: modules::builtin_modules(),
            enabled: Vec::new(),
            status: String::new(),
        };
        if let Some(root) = project_root {
            app.set_project(root);
        } else {
            app.status = "请先选择星火编辑器项目目录".to_string();
        }
        app
    }

    fn set_project(&mut self, root: PathBuf) {
        self.project_root = Some(root);
        self.refresh();
    }

    /// 重新定位 + 查询状态
    fn refresh(&mut self) {
        let Some(root) = self.project_root.clone() else {
            return;
        };
        match locate::locate(&root) {
            Ok(target) => {
                self.kernel_status = kernel::status(&target);
                self.has_backup = core::backup::has_backup(&target.backup_tag(), "isolation.lua");
                self.enabled = modules::enabled_modules(&target.common_dir);
                self.locate_error.clear();
                self.status = format!(
                    "已定位：编辑器版本 {}，script 包 {}",
                    target.api_version, target.script_version
                );
                self.target = Some(target);
            }
            Err(e) => {
                self.target = None;
                self.enabled.clear();
                self.kernel_status = kernel::KernelStatus::Unknown;
                self.locate_error = e.clone();
                self.status = format!("定位失败: {e}");
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
}

impl eframe::App for EditorPatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Kernel => self.ui_kernel(ui),
            Tab::Patches => self.ui_patches(ui),
            Tab::Help => Self::ui_help(ui),
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

    fn ui_kernel(&mut self, ui: &mut egui::Ui) {
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
        egui::Grid::new("kernel_info").num_columns(2).show(ui, |ui| {
            ui.label("编辑器版本：");
            ui.monospace(&target.api_version);
            ui.end_row();
            ui.label("编辑器目录：");
            ui.monospace(target.editor_root.display().to_string());
            ui.end_row();
            ui.label("script 包版本：");
            ui.monospace(target.script_version.to_string());
            ui.end_row();
            ui.label("common 目录：");
            ui.monospace(target.common_dir.display().to_string());
            ui.end_row();
            ui.label("内核状态：");
            match self.kernel_status {
                kernel::KernelStatus::NotApplied => {
                    ui.label("未应用");
                }
                kernel::KernelStatus::Applied => {
                    ui.colored_label(egui::Color32::from_rgb(0x3a, 0xa0, 0x50), "已应用");
                }
                kernel::KernelStatus::Unknown => {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xd0, 0x50, 0x50),
                        "无法识别（isolation.lua 缺失或格式不符）",
                    );
                }
            }
            ui.end_row();
            ui.label("源文件备份：");
            ui.label(if self.has_backup { "已备份" } else { "无" });
            ui.end_row();
        });
        ui.add_space(12.0);

        // 操作
        ui.horizontal(|ui| {
            if ui.button("应用补丁").clicked() {
                let Some(target) = &self.target else { return };
                match kernel::apply(target) {
                    Ok(msg) => self.status = format!("应用成功：{msg}（重启星火编辑器后生效）"),
                    Err(e) => self.status = format!("应用失败: {e}"),
                }
                self.refresh();
            }
            if ui.button("还原补丁").clicked() {
                let Some(target) = &self.target else { return };
                match kernel::restore(target) {
                    Ok(msg) => self.status = format!("还原成功：{msg}（重启星火编辑器后生效）"),
                    Err(e) => self.status = format!("还原失败: {e}"),
                }
                self.refresh();
            }
            if ui.button("刷新状态").clicked() {
                self.refresh();
            }
        });
        ui.add_space(8.0);
        ui.label("「应用补丁」会解锁 isolation.lua 中被禁用的函数并注入补丁框架入口；");
        ui.label("「还原补丁」用备份原样还原编辑器源文件，并移除全部补丁模块。");
        if let Ok(root) = core::backup::backup_root() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(format!("备份目录：{}", root.display()));
                if ui.button("打开").clicked() {
                    let _ = std::process::Command::new("explorer").arg(&root).spawn();
                }
            });
        }
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
        if self.kernel_status != kernel::KernelStatus::Applied {
            ui.label("内核补丁尚未应用，请先在「内核」标签页点击「应用补丁」。");
            ui.label("补丁模块依赖内核注入的框架入口才会被加载。");
            return;
        }

        ui.label("勾选启用补丁模块（重启星火编辑器后生效）：");
        ui.add_space(4.0);

        let mut toggled: Option<(usize, bool)> = None;
        for (idx, m) in self.modules.iter().enumerate() {
            let mut on = self.enabled.iter().any(|id| id == m.id);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut on, m.name).changed() {
                    toggled = Some((idx, on));
                }
                ui.monospace(format!("({})", m.id));
            });
            ui.indent(format!("desc_{}", m.id), |ui| {
                ui.label(m.description);
            });
            ui.add_space(4.0);
        }

        if let Some((idx, on)) = toggled {
            let Some(target) = &self.target else { return };
            let m = &self.modules[idx];
            match modules::set_module(&target.common_dir, m, on) {
                Ok(()) => {
                    self.status = format!(
                        "模块「{}」已{}（重启星火编辑器后生效）",
                        m.name,
                        if on { "启用" } else { "关闭" }
                    );
                }
                Err(e) => self.status = format!("操作失败: {e}"),
            }
            self.refresh();
        }
    }

    fn ui_help(ui: &mut egui::Ui) {
        ui.heading("帮助");
        ui.add_space(8.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                "【本应用的作用】\n\
                 编辑器补丁（sce_app_editor-patch）用于给星火编辑器打补丁，扩展编辑器自身能力：\n\
                 · 内核补丁：解锁编辑器脚本中被禁用的函数（io/os/debug 等），并注入补丁框架入口。\n\
                 · 补丁模块：在内核之上，可勾选启用/关闭的功能模块。\n\
                 \n\
                 【使用方法】\n\
                 1. 通过 bgd_sce_tools 的「应用 - 应用市场」安装本应用并打开（会自动传入当前项目）。\n\
                 2. 在「内核」标签页点击「应用补丁」。\n\
                 3. 在「补丁」标签页勾选需要的补丁模块。\n\
                 \n\
                 【重要】\n\
                 · 应用补丁 / 还原补丁 / 启停模块后，必须【重启星火编辑器】才能生效！\n\
                 · 首次应用补丁前会自动备份编辑器原始文件，「还原补丁」可随时恢复原状。\n\
                 · 星火编辑器更新（script 包版本变化）后补丁会失效，重新「应用补丁」即可。\n\
                 \n\
                 【安全性】\n\
                 · 修改编辑器源文件前必先备份（备份目录见「内核」标签页）。\n\
                 · 写入采用临时文件原子替换，避免写一半损坏编辑器。\n\
                 · 检测到文件格式不符会中止操作，不会蛮干。",
            );
        });
    }
}
