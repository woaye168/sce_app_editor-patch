//! 从项目路径定位星火编辑器的脚本包目录。
//!
//! 定位链：
//! 1. `<项目>/project/map_settings.json` → `api_version`（编辑器版本号，如 13）
//! 2. `<项目>/script/tsconfig.json` → `compilerOptions.typeRoots` 任意一条，
//!    形如 `D:/sce_online/Update/editor-pd.spark.xd.com/Res/_m/...`，截取编辑器根目录
//! 3. `<编辑器根>/api_pak_version.json` → `[api_version].<包名>` 拿包版本号，
//!    `#package_path.<包名>` 拿包路径前缀
//! 4. 包目录 = `<编辑器根>/<包路径前缀>/<版本>/<包名>`
//!    如 script 包 → `Res/_m/script/199/script`（其下 common 即 common 包）
//!    如 xdeditor 包 → `Res/_m/xdeditor/160/xdeditor`

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// 一次定位的全部结果（支持多库）
pub struct EditorTarget {
    /// 编辑器版本号（map_settings.json 的 api_version，如 "13"）
    pub api_version: String,
    /// 编辑器更新根目录（如 D:/sce_online/Update/editor-pd.spark.xd.com）
    pub editor_root: PathBuf,
    /// api_pak_version.json 全文（含 #package_path 与各版本包清单）
    pak: Value,
}

impl EditorTarget {
    /// 指定包的版本号（api_pak_version.json → [api_version][包名]）
    pub fn package_version(&self, name: &str) -> Result<u64, String> {
        self.pak
            .get(&self.api_version)
            .and_then(|v| v.get(name))
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                format!(
                    "api_pak_version.json 中找不到版本 {} 的 {} 包版本号",
                    self.api_version, name
                )
            })
    }

    /// 指定包的路径前缀（api_pak_version.json → #package_path[包名]，缺省 `Res/_m/<包名>`）
    fn package_path(&self, name: &str) -> String {
        self.pak
            .get("#package_path")
            .and_then(|m| m.get(name))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Res/_m/{name}"))
    }

    /// 指定包的真实目录：<编辑器根>/<包路径前缀>/<版本>/<包名>
    pub fn package_dir(&self, name: &str) -> Result<PathBuf, String> {
        let version = self.package_version(name)?;
        let dir = self
            .editor_root
            .join(self.package_path(name))
            .join(version.to_string())
            .join(name);
        if !dir.is_dir() {
            return Err(format!("{name} 包目录不存在: {}", dir.display()));
        }
        Ok(dir)
    }

    /// common 包目录（script 包下的 common 子目录）
    pub fn common_dir(&self) -> Result<PathBuf, String> {
        let dir = self.package_dir("script")?.join("common");
        if !dir.join("isolation.lua").is_file() {
            return Err(format!("common 包缺少 isolation.lua: {}", dir.display()));
        }
        Ok(dir)
    }

    /// 引擎运行根目录：editor_root 形如 <运行根>/Update/editor-pd.spark.xd.com，
    /// 运行根即 editor_root 的上两级。候选根下必须存在 version-<api_version> 目录，
    /// 否则返回明确错误（路径假设不符时静默回退会让后续 logs/dll 定位全错且不报错）。
    pub fn engine_root(&self) -> Result<PathBuf, String> {
        let candidate = self
            .editor_root
            .parent()
            .and_then(|p| p.parent())
            .ok_or_else(|| {
                format!(
                    "无法从编辑器根目录推导引擎运行根: {}",
                    self.editor_root.display()
                )
            })?;
        let version_dir = candidate.join(format!("version-{}", self.api_version));
        if !version_dir.is_dir() {
            return Err(format!(
                "引擎运行根校验失败（其下无 {} 目录）: {}",
                version_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                candidate.display()
            ));
        }
        Ok(candidate.to_path_buf())
    }

    /// 引擎版本目录：<运行根>/version-<api_version>（引擎 dll 所在处，如 version-13）
    pub fn version_dir(&self) -> Result<PathBuf, String> {
        Ok(self.engine_root()?.join(format!("version-{}", self.api_version)))
    }

    /// 备份分组：<api版本>/<包名_版本>，如 `api13/script_199`
    pub fn backup_group(&self, pkg: &str) -> Result<String, String> {
        Ok(format!(
            "api{}/{}_{}",
            self.api_version,
            pkg,
            self.package_version(pkg)?
        ))
    }
}

/// 判断一个路径是否是可用的 SCE 项目（含 project/map_settings.json）
pub fn is_valid_project(dir: &Path) -> bool {
    dir.join("project").join("map_settings.json").is_file()
}

pub fn locate(project_root: &Path) -> Result<EditorTarget, String> {
    let api_version = read_api_version(project_root)?;
    let editor_root = find_editor_root(project_root)?;
    let pak = read_api_pak(&editor_root)?;

    let target = EditorTarget {
        api_version,
        editor_root,
        pak,
    };
    // 校验最常用的 script/common 包可达
    target.common_dir()?;
    Ok(target)
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

/// 第 3 步：读 api_pak_version.json 全文
fn read_api_pak(editor_root: &Path) -> Result<Value, String> {
    let path = editor_root.join("api_pak_version.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", path.display()))
}

/// 去除 JSON 中的 `//` 行注释与 `/* */` 块注释（容忍手写 tsconfig），字符串内的内容不动。
/// 字节层剥离（注释定界符均为 ASCII，不会误伤 UTF-8 多字节序列），最后整体按 UTF-8 组装，
/// 非 ASCII 内容（如中文路径）原样保留。
fn strip_json_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c);
            if c == b'\\' && i + 1 < bytes.len() {
                out.push(bytes[i + 1]);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == b'"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }
        // 行注释
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // 块注释
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
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
    // 输入是合法 UTF-8 且只删除了完整 ASCII 注释段，输出必然仍是合法 UTF-8
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    //! 本机真实环境冒烟测试（只在装有星火编辑器的开发机上手工跑）：
    //! `cargo test -- --ignored`
    use super::*;

    #[test]
    fn test_engine_root_and_version_dir() {
        // 临时目录构造 <运行根>/Update/editor-pd.spark.xd.com 形态
        let base = std::env::temp_dir().join(format!("editor_patch_locate_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let editor_root = base.join("Update").join("editor-pd.spark.xd.com");
        std::fs::create_dir_all(&editor_root).unwrap();

        let target = EditorTarget {
            api_version: "13".to_string(),
            editor_root,
            pak: serde_json::json!({}),
        };
        // version-13 不存在：回退被校验拦截，报明确错误（含候选路径）
        let err = target.engine_root().unwrap_err();
        assert!(err.contains(base.to_string_lossy().as_ref()), "{err}");
        // 补齐 version-13 后正常推导
        std::fs::create_dir_all(base.join("version-13")).unwrap();
        assert_eq!(target.engine_root().unwrap(), base);
        assert_eq!(target.version_dir().unwrap(), base.join("version-13"));

        let _ = std::fs::remove_dir_all(&base);
    }

    /// strip_json_comments 必须原样保留非 ASCII 内容（如中文路径），且剥离两类注释
    #[test]
    fn test_strip_json_comments_utf8() {
        let src = "{\n\
            // 行注释：typeRoots\n\
            \"compilerOptions\": {\n\
            /* 块注释 */ \"typeRoots\": [\"D:/编辑器根目录/Update/editor-pd.spark.xd.com/Res/_m/script/\"]\n\
            }\n\
            }";
        let stripped = strip_json_comments(src);
        let json: Value = serde_json::from_str(&stripped).unwrap();
        let root = json["compilerOptions"]["typeRoots"][0].as_str().unwrap();
        assert_eq!(
            root,
            "D:/编辑器根目录/Update/editor-pd.spark.xd.com/Res/_m/script/"
        );
    }

    #[test]
    #[ignore]
    fn smoke_locate_real_project() {
        let project = Path::new("C:/Users/woaye/Documents/SCE Projects/test_res002");
        let target = locate(project).unwrap();
        assert_eq!(target.api_version, "13");
        assert!(target.package_version("script").unwrap() > 0);
        assert!(target.common_dir().unwrap().join("isolation.lua").is_file());
        let xdeditor = target.package_dir("xdeditor").unwrap();
        assert!(xdeditor.join("ui").join("menu_bar.lua").is_file());
        println!(
            "editor_root={} script={} xdeditor={}",
            target.editor_root.display(),
            target.package_version("script").unwrap(),
            target.package_version("xdeditor").unwrap(),
        );
    }
}
