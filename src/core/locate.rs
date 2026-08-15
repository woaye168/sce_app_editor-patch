//! 从项目路径定位星火编辑器的 common 脚本包目录。
//!
//! 定位链：
//! 1. `<项目>/project/map_settings.json` → `api_version`（编辑器版本号，如 13）
//! 2. `<项目>/script/tsconfig.json` → `compilerOptions.typeRoots` 任意一条，
//!    形如 `D:/sce_online/Update/editor-pd.spark.xd.com/Res/_m/...`，截取编辑器根目录
//! 3. `<编辑器根>/api_pak_version.json` → `[api_version].script` 拿到 script 包版本号
//! 4. common 目录 = `<编辑器根>/Res/_m/script/<script版本>/script/common`

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// 一次定位的全部结果
pub struct EditorTarget {
    /// 编辑器版本号（map_settings.json 的 api_version，如 "13"）
    pub api_version: String,
    /// 编辑器更新根目录（如 D:/sce_online/Update/editor-pd.spark.xd.com）
    pub editor_root: PathBuf,
    /// script（common）包版本号（如 199）
    pub script_version: u64,
    /// common 包目录（.../Res/_m/script/<ver>/script/common）
    pub common_dir: PathBuf,
}

impl EditorTarget {
    pub fn isolation_lua(&self) -> PathBuf {
        self.common_dir.join("isolation.lua")
    }

    /// 备份分组键：编辑器版本 + script 包版本
    pub fn backup_tag(&self) -> String {
        format!("api{}_script{}", self.api_version, self.script_version)
    }
}

/// 判断一个路径是否是可用的 SCE 项目（含 project/map_settings.json）
pub fn is_valid_project(dir: &Path) -> bool {
    dir.join("project").join("map_settings.json").is_file()
}

pub fn locate(project_root: &Path) -> Result<EditorTarget, String> {
    let api_version = read_api_version(project_root)?;
    let editor_root = find_editor_root(project_root)?;
    let script_version = read_script_version(&editor_root, &api_version)?;

    let common_dir = editor_root
        .join("Res")
        .join("_m")
        .join("script")
        .join(script_version.to_string())
        .join("script")
        .join("common");
    if !common_dir.join("isolation.lua").is_file() {
        return Err(format!(
            "common 包目录不存在或缺少 isolation.lua: {}",
            common_dir.display()
        ));
    }

    Ok(EditorTarget {
        api_version,
        editor_root,
        script_version,
        common_dir,
    })
}

/// 第 1 步：读 map_settings.json 的 api_version
/// 兼容两种形态：`"api_version": {"api_version": 13, ...}` 或 `"api_version": 13`
fn read_api_version(project_root: &Path) -> Result<String, String> {
    let path = project_root.join("project").join("map_settings.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", path.display()))?;
    match json.get("api_version") {
        Some(Value::Object(o)) => match o.get("api_version") {
            Some(Value::Number(n)) => Ok(n.to_string()),
            Some(Value::String(s)) => Ok(s.clone()),
            _ => Err("map_settings.json 的 api_version 对象中缺少 api_version 字段".to_string()),
        },
        Some(Value::Number(n)) => Ok(n.to_string()),
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err("map_settings.json 缺少 api_version 字段".to_string()),
    }
}

/// 第 2 步：从 tsconfig.json 的 typeRoots 提取编辑器根目录（`/Res/_m/` 之前的部分）
fn find_editor_root(project_root: &Path) -> Result<PathBuf, String> {
    let path = project_root.join("script").join("tsconfig.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let text = strip_json_comments(&text);
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", path.display()))?;

    // typeRoots 一般在 compilerOptions 下，容错也找顶层
    let type_roots = json
        .get("compilerOptions")
        .and_then(|c| c.get("typeRoots"))
        .or_else(|| json.get("typeRoots"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| "tsconfig.json 缺少 compilerOptions.typeRoots".to_string())?;

    for root in type_roots.iter().filter_map(|v| v.as_str()) {
        let normalized = root.replace('\\', "/");
        // 找 "/Res/_m/" 标记（大小写不敏感），其前缀即编辑器根目录
        let lower = normalized.to_lowercase();
        if let Some(idx) = lower.find("/res/_m/") {
            let prefix = normalized[..idx].trim_end_matches('/');
            if !prefix.is_empty() {
                return Ok(PathBuf::from(prefix));
            }
        }
    }
    Err("tsconfig.json 的 typeRoots 中没有包含 Res/_m 的路径，无法定位编辑器目录".to_string())
}

/// 第 3 步：api_pak_version.json → [api_version].script
fn read_script_version(editor_root: &Path, api_version: &str) -> Result<u64, String> {
    let path = editor_root.join("api_pak_version.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", path.display()))?;
    json.get(api_version)
        .and_then(|v| v.get("script"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            format!("api_pak_version.json 中找不到版本 {api_version} 的 script 包版本号")
        })
}

/// 去除 JSON 中的 `//` 行注释与 `/* */` 块注释（容忍手写 tsconfig），字符串内的内容不动
fn strip_json_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        // 行注释
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // 块注释
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    //! 本机真实环境冒烟测试（只在装有星火编辑器的开发机上手工跑）：
    //! `cargo test -- --ignored`
    use super::*;

    #[test]
    #[ignore]
    fn smoke_locate_real_project() {
        let project = Path::new(r"C:\Users\woaye\Documents\SCE Projects\test_res002");
        let target = locate(project).unwrap();
        assert_eq!(target.api_version, "13");
        assert!(target.script_version > 0);
        assert!(target.isolation_lua().is_file());
        println!(
            "editor_root={} script={} common={}",
            target.editor_root.display(),
            target.script_version,
            target.common_dir.display()
        );
    }
}
