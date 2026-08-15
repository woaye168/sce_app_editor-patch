//! 文件批量操作：整库解密、整目录复制，多线程并行 + 进度上报。
//!
//! 编辑器库里都是小文件（单库上千个），串行处理慢且阻塞 UI，
//! 这里用 std::thread::scope 按可用核数分块并行，进度通过原子计数上报。

use super::crypto;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 递归收集目录下所有 .lua 文件
pub fn collect_lua_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_by_ext(dir, "lua", &mut out);
    out.sort();
    out
}

/// 递归收集目录下所有文件
pub fn collect_all_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_by_ext(dir, "", &mut out);
    out.sort();
    out
}

fn collect_by_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_by_ext(&path, ext, out);
        } else if ext.is_empty() || path.extension().and_then(|e| e.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

/// 并行处理一批文件，`f` 返回 Err 会被收集（不中断其他文件），返回错误列表。
/// 每处理完一个文件 `done` 计数 +1。
pub fn parallel_for_each<F>(files: &[PathBuf], done: &Arc<AtomicUsize>, f: F) -> Vec<String>
where
    F: Fn(&Path) -> Result<(), String> + Sync,
{
    if files.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(files.len());
    let chunk = files.len().div_ceil(threads);
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for part in files.chunks(chunk) {
            let done = Arc::clone(done);
            let errors = &errors;
            let f = &f;
            s.spawn(move || {
                for file in part {
                    if let Err(e) = f(file) {
                        errors.lock().unwrap().push(e);
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    errors.into_inner().unwrap()
}

/// 解密单个文件（原地替换为明文源码）。
/// 已是明文的跳过；返回是否为本次新解密。
pub fn decrypt_file_in_place(path: &Path) -> Result<bool, String> {
    let raw = fs::read(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    if !crypto::is_encrypted(&raw) {
        return Ok(false); // 明文文件，不动
    }
    let plain = crypto::decrypt(&raw)?;
    crypto::write_atomic(path, &plain)?;
    Ok(true)
}

/// 递归复制整个目录（目标存在的文件被覆盖），`done` 每复制一个文件 +1
pub fn copy_dir_recursive(src: &Path, dst: &Path, done: &Arc<AtomicUsize>) -> Result<(), String> {
    let files = collect_all_files(src);
    parallel_for_each(&files, done, |file| {
        let rel = file
            .strip_prefix(src)
            .map_err(|e| format!("路径计算失败: {e}"))?;
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        fs::copy(file, &target)
            .map_err(|e| format!("复制 {} 失败: {e}", file.display()))?;
        Ok(())
    })
    .into_iter()
    .next()
    .map_or(Ok(()), Err)
}
