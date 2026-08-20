//! 插槽文件（编译期内嵌仓库 slots/ 目录）。
//!
//! 供 kernel 模块使用：精确版本整树字节复制；manifest 记录每个插槽文件的
//! 官方源哈希，供「最近低版本复用」判定。

use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

static SLOTS: Dir = include_dir!("$CARGO_MANIFEST_DIR/slots");

/// manifest 文件名（make_slots 生成，记录每个插槽文件的官方源哈希，复用判定用）
pub const MANIFEST_NAME: &str = "slot.manifest.json";

/// 插槽清单：slots/<pkg>/<ver>/slot.manifest.json
#[derive(Deserialize)]
pub struct SlotManifest {
    /// rel路径 → 条目
    pub files: BTreeMap<String, ManifestFile>,
}

#[derive(Deserialize)]
pub struct ManifestFile {
    /// slot=入口插槽注入 / unlock=解锁转换（保留字段，供排查/未来按类型差异化处理）
    #[serde(default)]
    #[allow(dead_code)]
    pub kind: String,
    /// 该插槽文件派生自的官方源文本哈希（解码+GBK→UTF-8 后）
    pub source_sha256: String,
}

/// 定位 slots/<pkg>/<version> 子目录。
/// include_dir 的 Dir::get_dir 只做单层名称匹配，嵌套子目录（如 script/199/common）
/// 的 path 带分隔符、get_dir 取不到，所以逐层下钻 + 名称比对。
fn slot_dir(pkg: &str, version: u64) -> Option<&'static Dir<'static>> {
    let ver = version.to_string();
    SLOTS.dirs()
        .find(|d| d.path().file_name().map(|n| n == pkg).unwrap_or(false))?
        .dirs()
        .find(|d| d.path().file_name().map(|n| n == ver.as_str()).unwrap_or(false))
}

/// 是否有指定库/版本的插槽文件
pub fn has_slots(pkg: &str, version: u64) -> bool {
    slot_dir(pkg, version).is_some()
}

/// 指定版本插槽树是否包含某个相对路径文件（如 pie_capture 的 ui/gameplay_in_editor_view.lua）
pub fn has_slot_file(pkg: &str, version: u64, rel: &str) -> bool {
    slot_dir(pkg, version)
        .and_then(|d| d.get_file(rel))
        .is_some()
}

/// 该库所有带插槽的版本中，低于 current 的版本号列表（降序，复用判定从最近低版本开始）
pub fn versions_below(pkg: &str, current: u64) -> Vec<u64> {
    let mut vers: Vec<u64> = SLOTS
        .dirs()
        .find(|d| d.path().file_name().map(|n| n == pkg).unwrap_or(false))
        .map(|pkg_dir| {
            pkg_dir
                .dirs()
                .filter_map(|d| {
                    d.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .filter(|v| *v < current)
                })
                .collect()
        })
        .unwrap_or_default();
    vers.sort_unstable_by(|a, b| b.cmp(a));
    vers
}

/// 读取指定版本的插槽清单（无 manifest 的老版本 slots 返回 None，复用判定跳过）
pub fn manifest(pkg: &str, version: u64) -> Option<SlotManifest> {
    let dir = slot_dir(pkg, version)?;
    let file = dir.files().find(|f| {
        f.path()
            .file_name()
            .map(|n| n == MANIFEST_NAME)
            .unwrap_or(false)
    })?;
    let text = std::str::from_utf8(file.contents()).ok()?;
    serde_json::from_str(text).ok()
}

/// 把 slots/<pkg>/<version>/ 整树覆盖写入包目录（manifest 文件本身不落地），返回写入文件数
pub fn apply_slots(pkg: &str, version: u64, pkg_dir: &Path) -> Result<usize, String> {
    let dir = slot_dir(pkg, version)
        .ok_or_else(|| format!("无 {pkg} v{version} 的插槽文件"))?;
    let mut count = 0;
    write_dir(dir, pkg_dir, &mut count)?;
    Ok(count)
}

/// file.path() 相对 slots 根（含 <pkg>/<version>/ 前缀），写入时剥掉前缀
fn write_dir(dir: &Dir, dest: &Path, count: &mut usize) -> Result<(), String> {
    for file in dir.files() {
        // manifest 是元数据，不写入包目录
        if file.path().file_name().map(|n| n == MANIFEST_NAME).unwrap_or(false) {
            continue;
        }
        let rel = file.path();
        // 剥掉前两层（pkg/version）
        let rel = rel.iter().skip(2).collect::<std::path::PathBuf>();
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        super::crypto::write_atomic(&target, file.contents())?;
        *count += 1;
    }
    for sub in dir.dirs() {
        write_dir(sub, dest, count)?;
    }
    Ok(())
}
