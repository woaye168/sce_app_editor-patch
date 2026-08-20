//! 内核补丁：按库处理（0.3.1 复制式插槽架构）。
//!
//! 对每个目标库（`LIBS` 登记表）执行：
//! 1. **整库备份**（仅首次，见 backup 模块）
//! 2. **整库解密**：库内加密的 .lua 原地替换为明文源码（明文跳过）。多线程并行 + 进度上报。
//! 3. **应用插槽文件**：把仓库内嵌的 `slots/<库>/<版本>/` 整树复制覆盖进库目录。
//!    插槽文件是「完整新源码」（官方源码 + 插槽/修改），字节级复制，不做编解码。
//!    无匹配版本子目录则跳过并明确提示（不蛮干）。
//! 4. **补丁目录**：在库 require 根下创建 sce_app_editor-patch/，首次创建启用默认勾选模块
//!
//! 「还原补丁」：有备份的库整树覆盖还原 + 删除补丁目录。
//! 「状态检查」：插槽标记（+script 库解锁标记）是否在，编辑器升级覆盖后显示「未应用」。
//!
//! 本文件为内核聚合入口：库登记表 / 状态检查 / 进度句柄；
//! 应用/还原流程在 kernel/{apply,restore}.rs，内嵌插槽文件在 core/slots.rs。

use super::locate::EditorTarget;
use super::{backup, crypto, slot_inject, slots};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

mod apply;
mod restore;
#[cfg(test)]
mod tests;

pub use apply::apply_async;
pub use restore::restore_async;

/// 插槽标记（插槽文件内含此注释即视为已应用）
pub const INJECT_MARK: &str = "-->> sce_app_editor-patch >>";
/// 解锁行标记前缀（script 库 isolation.lua 状态校验用）
pub const UNLOCK_MARK: &str = "-- [sce_app_editor-patch 解锁] ";

/// 一个目标库（补丁打在该库入口点）
pub struct LibSpec {
    /// 包名（api_pak_version.json 中的键）
    pub pkg: &'static str,
    /// 界面显示名
    pub name: &'static str,
    /// 库内 require 根（相对包目录）：package.path 指向的目录
    pub require_root: &'static str,
    /// 入口文件（相对包目录）：状态检查读取此文件确认插槽
    pub entry: &'static str,
}

/// 全部目标库（新增库补丁在此登记；插槽文件在 slots/<pkg>/<版本>/ 下用 include_dir 内嵌）
pub const LIBS: &[LibSpec] = &[
    LibSpec {
        pkg: "script",
        name: "script（游戏脚本/common 包）",
        require_root: "common",
        entry: "common/init.lua",
    },
    LibSpec {
        pkg: "xdeditor",
        name: "xdeditor（编辑器界面）",
        require_root: "",
        entry: "main.lua",
    },
];

impl LibSpec {
    pub fn package_dir(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        target.package_dir(self.pkg)
    }

    /// 库 require 根目录（补丁目录所在处）
    pub fn require_root_dir(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        Ok(self.package_dir(target)?.join(self.require_root))
    }

    fn entry_path(&self, target: &EditorTarget) -> Result<PathBuf, String> {
        Ok(self.package_dir(target)?.join(self.entry))
    }

    fn backup_group(&self, target: &EditorTarget) -> Result<String, String> {
        target.backup_group(self.pkg)
    }
}

/// 单个库的状态
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum LibStatus {
    /// 已应用（入口插槽在位，script 库还要求 isolation 解锁标记在位）
    Applied,
    /// 未应用（原始状态，或被编辑器升级覆盖）
    NotApplied,
    /// 包目录/入口文件缺失
    Missing,
}

/// 插槽可用级别（0.5.3 F4 三级回退）
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SlotLevel {
    /// 精确版本插槽：slots/<pkg>/<版本>/ 存在，整树字节复制
    Exact,
    /// 可复用：最近低版本插槽的官方源哈希与新版本一致，直接复用
    Reusable(u64),
    /// 可运行时注入：无精确/可复用插槽，应用时对新版本源现场注入
    Injectable,
    /// 入口文件缺失，无法注入
    Missing,
}

impl SlotLevel {
    /// UI 展示文案（仅在未应用状态下提示用）
    pub fn hint(&self) -> String {
        match self {
            SlotLevel::Exact => String::new(),
            SlotLevel::Reusable(v) => format!("（无本版本精确插槽，可复用 v{v}）"),
            SlotLevel::Injectable => "（无精确插槽，将运行时注入）".to_string(),
            SlotLevel::Missing => "（入口缺失，无法应用插槽）".to_string(),
        }
    }
}

/// 单个库的完整状态（供 UI 展示）
pub struct LibStatusInfo {
    pub pkg: &'static str,
    pub label: &'static str,
    pub version: String,
    pub status: LibStatus,
    pub has_backup: bool,
    /// 插槽可用级别（三级回退判定）
    pub slot_level: SlotLevel,
    pub path: String,
}

/// 判定库的插槽可用级别（三级回退，供状态展示与应用流程共用）
fn slot_level(lib: &LibSpec, target: &EditorTarget, version: u64) -> SlotLevel {
    if slots::has_slots(lib.pkg, version) {
        return SlotLevel::Exact;
    }
    let Ok(pkg_dir) = lib.package_dir(target) else {
        return SlotLevel::Missing;
    };
    if !pkg_dir.join(lib.entry).is_file() {
        return SlotLevel::Missing;
    }
    if let Some(v) = find_reusable_version(lib.pkg, &pkg_dir, version) {
        return SlotLevel::Reusable(v);
    }
    SlotLevel::Injectable
}

/// pie_capture 拍照修复的行为主体是插槽文件 ui/gameplay_in_editor_view.lua（仅随
/// xdeditor v169 slots 下发）。其他版本启用 pie_capture 模块不会生效——本函数按实际
/// 应用路径（精确 → 复用 → 运行时注入，注入只做入口/isolation 不含行为插槽）判定，
/// 缺失时返回明确提示文案，不允许静默。
pub fn pie_capture_slot_warning(target: &EditorTarget) -> Option<String> {
    const REL: &str = "ui/gameplay_in_editor_view.lua";
    let version = target.package_version("xdeditor").ok()?;
    let slot_src = if slots::has_slots("xdeditor", version) {
        Some(version)
    } else {
        target
            .package_dir("xdeditor")
            .ok()
            .and_then(|dir| find_reusable_version("xdeditor", &dir, version))
    };
    let missing = match slot_src {
        Some(v) => !slots::has_slot_file("xdeditor", v, REL),
        None => true,
    };
    missing.then(|| {
        format!("当前编辑器版本（xdeditor v{version}）不支持拍照修复（pie_capture 行为插槽缺失，模块可启用但不生效）")
    })
}

/// 找可复用的最近低版本：带 manifest 且其记录的官方源哈希与新版本解码源全部一致
fn find_reusable_version(pkg: &str, pkg_dir: &Path, current: u64) -> Option<u64> {
    for v in slots::versions_below(pkg, current) {
        let Some(manifest) = slots::manifest(pkg, v) else {
            continue;
        };
        let all_match = manifest.files.iter().all(|(rel, f)| {
            let Ok(raw) = std::fs::read(pkg_dir.join(rel)) else {
                return false;
            };
            slot_inject::source_hash(&slot_inject::decode_source(&raw)) == f.source_sha256
        });
        if all_match {
            return Some(v);
        }
    }
    None
}

/// 检查全部目标库状态
pub fn check(target: &EditorTarget) -> Vec<LibStatusInfo> {
    LIBS.iter()
        .map(|lib| {
            let version = target.package_version(lib.pkg).ok();
            let version_str = version.map(|v| v.to_string()).unwrap_or_else(|| "?".to_string());
            let has_backup = lib
                .backup_group(target)
                .map(|g| backup::has_backup(&target.editor_root, &g))
                .unwrap_or(false);
            let level = version
                .map(|v| slot_level(lib, target, v))
                .unwrap_or(SlotLevel::Missing);
            let (path, status) = match lib.entry_path(target) {
                Ok(entry) => {
                    // read_lua 自适应加密/明文：未打补丁的加密入口也能正确读出
                    let s = match crypto::read_lua(&entry) {
                        Ok(text) if text.contains(INJECT_MARK) => {
                            if lib.pkg == "script" {
                                match lib.package_dir(target).map(|d| d.join("common/isolation.lua")) {
                                    Ok(iso) => match crypto::read_lua(&iso) {
                                        Ok(t) if t.contains(UNLOCK_MARK) => LibStatus::Applied,
                                        Ok(_) => LibStatus::NotApplied,
                                        Err(_) => LibStatus::Missing,
                                    },
                                    Err(_) => LibStatus::Missing,
                                }
                            } else {
                                LibStatus::Applied
                            }
                        }
                        Ok(_) => LibStatus::NotApplied,
                        Err(_) => LibStatus::Missing,
                    };
                    (entry.display().to_string(), s)
                }
                Err(e) => (e, LibStatus::Missing),
            };
            LibStatusInfo {
                pkg: lib.pkg,
                label: lib.name,
                version: version_str,
                status,
                has_backup,
                slot_level: level,
                path,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- 进度

/// 后台任务进度（应用/还原共用）
pub struct TaskProgress {
    pub phase: String,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub finished: bool,
    pub ok: bool,
    pub summary: String,
}

pub type SharedProgress = Arc<Mutex<TaskProgress>>;

fn new_progress() -> SharedProgress {
    Arc::new(Mutex::new(TaskProgress {
        phase: "准备中…".to_string(),
        total: 0,
        done: Arc::new(AtomicUsize::new(0)),
        finished: false,
        ok: false,
        summary: String::new(),
    }))
}

fn set_phase(progress: &SharedProgress, phase: impl Into<String>) {
    progress.lock().unwrap().phase = phase.into();
}

fn set_total(progress: &SharedProgress, total: usize) {
    progress.lock().unwrap().total = total;
}
