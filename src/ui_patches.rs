//! 「补丁」标签页 UI（EditorPatchApp::ui_patches，自 main.rs 拆出）。

use crate::EditorPatchApp;
use sce_app_editor_patch::core::{kernel, modules};

impl EditorPatchApp {
    pub(crate) fn ui_patches(&mut self, ui: &mut egui::Ui) {
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
                        let vdir = target.version_dir()?;
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
                    // pie_capture 行为插槽仅随 xdeditor v169 下发，其他版本启用不生效——明确提示
                    if on && m.id == "pie_capture" {
                        if let Some(w) = kernel::pie_capture_slot_warning(target) {
                            self.status = w.clone();
                            self.log("WARN", &w);
                        }
                    }
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
}
