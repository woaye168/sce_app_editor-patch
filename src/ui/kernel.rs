//! 「内核」标签页 UI（EditorPatchApp::ui_kernel，自 main.rs 拆出）。

use crate::EditorPatchApp;
use sce_app_editor_patch::core::{editor, kernel, log};

impl EditorPatchApp {
    pub(crate) fn ui_kernel(&mut self, ui: &mut egui::Ui, task_running: bool) {
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
}
