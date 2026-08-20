//! 「设置」标签页 UI（EditorPatchApp::ui_settings，自 main.rs 拆出）。

use crate::EditorPatchApp;
use sce_app_editor_patch::core::editor;

impl EditorPatchApp {
    /// 「设置」标签页：MCP 服务端口配置
    pub(crate) fn ui_settings(&mut self, ui: &mut egui::Ui) {
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
                    match target
                        .engine_root()
                        .and_then(|r| editor::set_editor_exe_name(&r, &name))
                    {
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
}
