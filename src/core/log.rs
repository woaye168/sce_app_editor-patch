//! 日志：按日期分文件的应用日志（bgd_appsdk 统一实现，本模块为应用名薄封装）。
//! 路径：`<项目>/.bgd/log/app_editor-patch-YYYY-MM-DD.log`（无 .bgd 退回引擎补丁目录）。

use std::path::{Path, PathBuf};

const APP_NAME: &str = "app_editor-patch";

/// 日志文件路径（按当天日期）
pub fn log_path(project_root: Option<&Path>, editor_root: Option<&Path>) -> PathBuf {
    bgd_appsdk::log::log_path(APP_NAME, project_root, editor_root)
}

/// 追加一行日志（失败静默，不影响主流程）
pub fn log(project_root: Option<&Path>, editor_root: Option<&Path>, level: &str, message: &str) {
    bgd_appsdk::log::log(APP_NAME, project_root, editor_root, level, message)
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_log_path() {
        let p = super::log_path(None, None);
        assert!(p.display().to_string().contains("app_editor-patch-"));
    }
}
