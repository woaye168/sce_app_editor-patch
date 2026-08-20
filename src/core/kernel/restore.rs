//! 「还原补丁」流程：有备份的库整树覆盖还原 + 删除补丁目录 + 恢复 sce.deps.json。

use super::{new_progress, set_phase, set_total, LibSpec, SharedProgress, LIBS};
use crate::core::{backup, bridge_deploy, locate, log, modules, ops};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 后台线程执行「还原补丁」
pub fn restore_async(project_root: PathBuf) -> SharedProgress {
    let progress = new_progress();
    let p = Arc::clone(&progress);
    std::thread::spawn(move || {
        let result = restore_all(&project_root, &p);
        let mut g = p.lock().unwrap();
        match result {
            Ok(summary) => {
                g.ok = true;
                g.summary = summary;
            }
            Err(e) => {
                g.ok = false;
                g.summary = e;
            }
        }
        g.finished = true;
    });
    progress
}

pub(crate) fn restore_all(project_root: &Path, progress: &SharedProgress) -> Result<String, String> {
    set_phase(progress, "定位编辑器…");
    let target = locate::locate(project_root)?;
    log::log(Some(project_root), Some(&target.editor_root), "INFO", "开始还原补丁");

    struct RJob<'a> {
        lib: &'a LibSpec,
        dir: PathBuf,
        group: String,
        file_count: usize,
    }
    let mut jobs = Vec::new();
    for lib in LIBS {
        let (Ok(dir), Ok(group)) = (lib.package_dir(&target), lib.backup_group(&target)) else {
            continue;
        };
        if !backup::has_backup(&target.editor_root, &group) {
            continue;
        }
        let count = ops::collect_all_files(&dir).len();
        jobs.push(RJob { lib, dir, group, file_count: count });
    }
    if jobs.is_empty() {
        return Err("没有任何备份可用于还原（尚未应用过补丁）".to_string());
    }
    set_total(progress, jobs.iter().map(|j| j.file_count).sum());
    let done = progress.lock().unwrap().done.clone();

    let mut lines: Vec<String> = Vec::new();
    for job in &jobs {
        set_phase(progress, format!("还原 {} 库…", job.lib.pkg));
        match backup::restore_lib(&target.editor_root, &job.group, &job.dir, &done) {
            Ok(()) => {
                if let Ok(root) = job.lib.require_root_dir(&target) {
                    let patch_dir = modules::patch_dir(&root);
                    if patch_dir.exists() {
                        std::fs::remove_dir_all(&patch_dir)
                            .map_err(|e| format!("删除 {} 失败: {e}", patch_dir.display()))?;
                    }
                }
                lines.push(format!("✔ {}：已用备份整库还原", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "INFO",
                    &format!("库[{}]还原成功", job.lib.pkg));
            }
            Err(e) => {
                lines.push(format!("✘ {}：{e}", job.lib.name));
                log::log(Some(project_root), Some(&target.editor_root), "ERROR",
                    &format!("库[{}]还原失败: {e}", job.lib.pkg));
            }
        }
    }

    // 追加恢复 sce.deps.json（若曾部署 MCP 桥）：失败只记日志，不中断整体还原
    match target.version_dir().and_then(|vdir| bridge_deploy::restore_deps(&vdir)) {
        Ok(()) => {
            log::log(Some(project_root), Some(&target.editor_root), "INFO",
                "sce.deps.json 恢复检查完成");
        }
        Err(e) => {
            lines.push(format!("✘ 恢复 sce.deps.json：{e}"));
            log::log(Some(project_root), Some(&target.editor_root), "ERROR",
                &format!("恢复 sce.deps.json 失败: {e}"));
        }
    }
    Ok(lines.join("\n"))
}
