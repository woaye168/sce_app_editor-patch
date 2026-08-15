//! bgd_mcp_bridge.dll 部署/摘除与 sce.deps.json 登记/恢复。
//!
//! 编辑器引擎运行目录结构：`<运行根>/version-<api>/` 下是引擎 dll（sce.dll、sce.deps.json 等）。
//! 「部署」= 把编译期内嵌的 bgd_mcp_bridge.dll 写入该目录，并在 sce.deps.json 里登记程序集
//! 条目（targets + libraries 两处），.NET 加载器才能解析我们的程序集。
//!
//! 安全约定：
//! - 首次注入 sce.deps.json 前备份原文为 sce.deps.json_bak（已存在则不动，保证备份是最原始状态）；
//! - 写盘一律原子替换（crypto::write_atomic）；
//! - 「还原补丁」用 restore_deps 把备份原样写回并删除 dll，保证字节级还原。

use super::crypto;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

/// 编译期内嵌的桥接 dll（预编译产物，csharp/bgd_mcp_bridge 构建输出）
pub const BRIDGE_DLL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/csharp/bgd_mcp_bridge/bin/x64/Release/bgd_mcp_bridge.dll"
));

/// deps.json targets 中的目标组名（.NET 9 / win-x64）
const TARGET_GROUP: &str = ".NETCoreApp,Version=v9.0/win-x64";
/// 程序集登记键（targets 与 libraries 两处共用）
const LIB_KEY: &str = "bgd_mcp_bridge/1.0.0";

fn dll_path(version_dir: &Path) -> std::path::PathBuf {
    version_dir.join("bgd_mcp_bridge.dll")
}

fn deps_path(version_dir: &Path) -> std::path::PathBuf {
    version_dir.join("sce.deps.json")
}

fn deps_bak_path(version_dir: &Path) -> std::path::PathBuf {
    version_dir.join("sce.deps.json_bak")
}

/// 部署：写入 dll + 向 sce.deps.json 注入登记条目（幂等）
///
/// dll 写入策略：已部署 dll 与内嵌版本**内容一致则跳过**（避免不必要写入）；
/// 内容不同（应用升级带来新 dll）才覆盖写入。覆盖时若编辑器正在运行（dll 被进程锁定），
/// 写入会失败——此时返回明确错误提示用户关闭编辑器后重试。
pub fn deploy(version_dir: &Path) -> Result<(), String> {
    // a) 原子写入 dll（内容不同才写）
    let dll = dll_path(version_dir);
    if needs_dll_update(&dll) {
        write_dll(&dll)?;
    }

    // b) deps.json 注入
    let deps = deps_path(version_dir);
    if !deps.is_file() {
        return Err(format!("sce.deps.json 不存在: {}", deps.display()));
    }
    let bak = deps_bak_path(version_dir);
    // 首次注入前备份原文；已存在备份则不动（保证备份始终是最原始状态）
    if !bak.exists() {
        fs::copy(&deps, &bak).map_err(|e| format!("备份 sce.deps.json 失败: {e}"))?;
    }

    let text = fs::read_to_string(&deps).map_err(|e| format!("读取 {} 失败: {e}", deps.display()))?;
    let mut doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", deps.display()))?;

    // targets[".NETCoreApp,Version=v9.0/win-x64"]["bgd_mcp_bridge/1.0.0"]（已存在则跳过）
    let targets = doc
        .get_mut("targets")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "sce.deps.json 缺少 targets 对象".to_string())?;
    let group = targets
        .entry(TARGET_GROUP.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let group = group
        .as_object_mut()
        .ok_or_else(|| format!("sce.deps.json targets[\"{TARGET_GROUP}\"] 不是对象"))?;
    group.entry(LIB_KEY.to_string()).or_insert_with(|| {
        json!({
            "runtime": {
                "bgd_mcp_bridge.dll": {
                    "assemblyVersion": "1.0.0.0",
                    "fileVersion": "1.0.0.0"
                }
            }
        })
    });

    // libraries["bgd_mcp_bridge/1.0.0"]（已存在则跳过）
    let libraries = doc
        .get_mut("libraries")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| "sce.deps.json 缺少 libraries 对象".to_string())?;
    libraries.entry(LIB_KEY.to_string()).or_insert_with(|| {
        json!({"type": "project", "serviceable": false, "sha512": ""})
    });

    let out =
        serde_json::to_string_pretty(&doc).map_err(|e| format!("序列化 sce.deps.json 失败: {e}"))?;
    crypto::write_atomic(&deps, out.as_bytes())
}

/// 判断已部署 dll 是否需要更新（不存在或与内嵌版本内容不同）
fn needs_dll_update(dll: &Path) -> bool {
    if !dll.is_file() {
        return true;
    }
    match fs::read(dll) {
        Ok(existing) => existing != BRIDGE_DLL,
        // 读不到（可能被占用）时保守视为需更新，让写入阶段给出明确占用错误
        Err(_) => true,
    }
}

/// 覆盖写入 dll；被运行中的编辑器锁定（占用）时给出明确中文提示
fn write_dll(dll: &Path) -> Result<(), String> {
    crypto::write_atomic(dll, BRIDGE_DLL).map_err(|e| {
        format!(
            "写入 {} 失败: {e}。若星火编辑器正在运行会锁定该 dll，请关闭编辑器后重试。",
            dll.display()
        )
    })
}

/// 检测模块勾选状态下 dll 是否需要重新部署（dll 缺失或版本与内嵌不一致）。
/// 用于应用升级后：模块勾选状态保留，但内嵌 dll 已更新，需提示/自动重部署。
pub fn needs_redeploy(version_dir: &Path) -> bool {
    needs_dll_update(&dll_path(version_dir))
}

/// 摘除：从 sce.deps.json 移除登记条目（没有则跳过）+ 删除 dll。
/// 注意：不删除 sce.deps.json_bak（那是「还原补丁」用的原始备份）。
pub fn undeploy(version_dir: &Path) -> Result<(), String> {
    let deps = deps_path(version_dir);
    if deps.is_file() {
        let text =
            fs::read_to_string(&deps).map_err(|e| format!("读取 {} 失败: {e}", deps.display()))?;
        let mut doc: Value =
            serde_json::from_str(&text).map_err(|e| format!("解析 {} 失败: {e}", deps.display()))?;

        let mut changed = false;
        if let Some(group) = doc
            .get_mut("targets")
            .and_then(|t| t.get_mut(TARGET_GROUP))
            .and_then(|g| g.as_object_mut())
        {
            if group.remove(LIB_KEY).is_some() {
                changed = true;
            }
        }
        if let Some(libraries) = doc.get_mut("libraries").and_then(|l| l.as_object_mut()) {
            if libraries.remove(LIB_KEY).is_some() {
                changed = true;
            }
        }
        if changed {
            let out = serde_json::to_string_pretty(&doc)
                .map_err(|e| format!("序列化 sce.deps.json 失败: {e}"))?;
            crypto::write_atomic(&deps, out.as_bytes())?;
        }
    }

    let dll = dll_path(version_dir);
    if dll.exists() {
        fs::remove_file(&dll).map_err(|e| format!("删除 {} 失败: {e}", dll.display()))?;
    }
    Ok(())
}

/// 还原（「还原补丁」时调用）：sce.deps.json_bak 存在则原子复制回 sce.deps.json 并删除备份，
/// 同时删除 dll；两者都不存在则跳过。
pub fn restore_deps(version_dir: &Path) -> Result<(), String> {
    let bak = deps_bak_path(version_dir);
    if bak.is_file() {
        let bytes = fs::read(&bak).map_err(|e| format!("读取 {} 失败: {e}", bak.display()))?;
        crypto::write_atomic(&deps_path(version_dir), &bytes)?;
        fs::remove_file(&bak).map_err(|e| format!("删除 {} 失败: {e}", bak.display()))?;
    }

    let dll = dll_path(version_dir);
    if dll.exists() {
        fs::remove_file(&dll).map_err(|e| format!("删除 {} 失败: {e}", dll.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 迷你 sce.deps.json：两个 targets 组 + libraries
    const MINI_DEPS: &str = r#"{
  "runtimeTarget": {
    "name": ".NETCoreApp,Version=v9.0/win-x64"
  },
  "targets": {
    ".NETCoreApp,Version=v9.0/win-x64": {
      "sce/1.0.0": {
        "runtime": {
          "sce.dll": {}
        }
      }
    },
    ".NETCoreApp,Version=v9.0": {
      "sce/1.0.0": {}
    }
  },
  "libraries": {
    "sce/1.0.0": {
      "type": "project",
      "serviceable": false,
      "sha512": ""
    }
  }
}"#;

    fn setup() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("editor_patch_bridge_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("sce.deps.json"), MINI_DEPS).unwrap();
        dir
    }

    #[test]
    fn test_deploy_undeploy_restore() {
        let dir = setup();
        let dll = dir.join("bgd_mcp_bridge.dll");
        let bak = dir.join("sce.deps.json_bak");

        // deploy：条目注入 + dll 写入 + 首次备份
        deploy(&dir).unwrap();
        assert!(dll.is_file());
        assert_eq!(fs::read(&dll).unwrap(), BRIDGE_DLL);
        assert_eq!(fs::read_to_string(&bak).unwrap(), MINI_DEPS);
        let text = fs::read_to_string(dir.join("sce.deps.json")).unwrap();
        let doc: Value = serde_json::from_str(&text).unwrap();
        let entry = &doc["targets"][TARGET_GROUP][LIB_KEY];
        assert_eq!(entry["runtime"]["bgd_mcp_bridge.dll"]["assemblyVersion"], "1.0.0.0");
        assert_eq!(doc["libraries"][LIB_KEY]["type"], "project");
        // 另一个 targets 组不受影响
        assert!(doc["targets"][".NETCoreApp,Version=v9.0"].get(LIB_KEY).is_none());

        // 再 deploy：幂等（备份仍是原文，内容不重复）
        deploy(&dir).unwrap();
        assert_eq!(fs::read_to_string(&bak).unwrap(), MINI_DEPS);
        let text2 = fs::read_to_string(dir.join("sce.deps.json")).unwrap();
        assert_eq!(text2.matches(LIB_KEY).count(), 2); // targets + libraries 各一处

        // undeploy：条目摘除 + dll 删除，备份保留
        undeploy(&dir).unwrap();
        assert!(!dll.exists());
        assert!(bak.is_file());
        let doc: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("sce.deps.json")).unwrap(),
        )
        .unwrap();
        assert!(doc["targets"][TARGET_GROUP].get(LIB_KEY).is_none());
        assert!(doc["libraries"].get(LIB_KEY).is_none());

        // 再 undeploy：幂等不报错
        undeploy(&dir).unwrap();

        // 重新 deploy 后 restore_deps：内容与原始字节一致、dll 与备份都清掉
        deploy(&dir).unwrap();
        restore_deps(&dir).unwrap();
        assert_eq!(fs::read_to_string(dir.join("sce.deps.json")).unwrap(), MINI_DEPS);
        assert!(!dll.exists());
        assert!(!bak.exists());

        // 再 restore_deps：无备份无 dll，跳过不报错
        restore_deps(&dir).unwrap();

        let _ = fs::remove_dir_all(&dir);
    }
}
