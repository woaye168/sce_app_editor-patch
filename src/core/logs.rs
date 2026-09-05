//! 日志读取（0.8.0 自 editor.rs 拆出，单文件职责纪律）。
//!
//! - get_game_logs：读 <运行根>/logs/ 下游戏客户端/服务端/bgd_csharp 最新日志文件信息（离线可用）
//! - match 正则过滤 + errors 段自动上浮 + 同条收纳（×N 计数）——防刷屏灌爆上下文、
//!   防前置错误导致假阳性/假阴性漏判（机制注释见 scan_log_lines）

use super::locate;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 日志源定义：(key, 子目录, 文件前缀, 说明) —— 每类取最新一个文件（0.5.6 起带 desc 说明）
const LOG_SOURCES: &[(&str, &str, &str, &str)] = &[
    ("bridge_audit", "bgd_csharp", "audit-", "MCP桥审计日志（编辑器补丁 bgd_mcp_bridge 的 write/danger 能力调用审计）"),
    ("bridge_main", "bgd_csharp", "bgd_csharp-", "MCP桥入口日志（编辑器补丁 bgd_mcp_bridge 服务启动/请求处理日志）"),
    ("xdeditor_client", "lua", "lua-editor-", "编辑器日志（星火编辑器操作动作及运行日志）"),
    ("game_client", "lua", "lua-game-", "游戏客户端日志（调试游戏后产生）"),
    ("service_core", "server", "core-game-server-", "服务器底层日志（游戏服务端底层框架日志）"),
    ("game_server", "server", "lua-game-server-", "游戏服务端日志（游戏服务端自身代码日志）"),
];

/// 对外输出路径统一正斜杠（0.5.6 R2）
pub fn to_slash(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// 获取游戏日志（离线可用；默认只返回文件路径与信息，tail_lines>0 才带内容）。
/// source 取值：具体 key（见 LOG_SOURCES）/ 动态聚合前缀（如 bridge 命中全部 bridge_*）/
/// all / 缺省 game（命中 game_client + game_server）。
/// match_pattern（0.8.0 R3）：正则过滤命中行（同条收纳+计数）+ errors/warns 段自动上浮；
/// 与 tail_lines 可同时用（互补不互斥）。ignore_case（0.8.10 FR-02）：match 不区分大小写。
pub fn get_game_logs(
    project_root: &Path,
    source: &str,
    tail_lines: usize,
    match_pattern: Option<&str>,
    ignore_case: bool,
) -> Result<Value, String> {
    let target = locate::locate(project_root)?;
    let logs_root = target.engine_root()?.join("logs");

    let match_re = match match_pattern {
        Some(p) if !p.trim().is_empty() => Some(
            regex::RegexBuilder::new(p)
                .case_insensitive(ignore_case)
                .build()
                .map_err(|e| format!("match 正则无效（{e}），请修正后重试"))?,
        ),
        _ => None,
    };

    let source = if source.trim().is_empty() { "game" } else { source.trim() };
    // 匹配规则：all=全部；精确 key；否则动态聚合（key 以 `source_` 为前缀）
    let matched: Vec<&(&str, &str, &str, &str)> = LOG_SOURCES
        .iter()
        .filter(|(key, _, _, _)| {
            source == "all" || *key == source || key.starts_with(&format!("{source}_"))
        })
        .collect();
    if matched.is_empty() {
        let keys: Vec<&str> = LOG_SOURCES.iter().map(|(k, _, _, _)| *k).collect();
        return Err(format!(
            "source '{source}' 未匹配到任何日志项（可用 key：{}；也可用前缀聚合如 bridge/game、或 all）",
            keys.join(" / ")
        ));
    }

    let mut out = serde_json::Map::new();
    for (key, sub, prefix, desc) in matched {
        let dir = logs_root.join(sub);
        match latest_file(&dir, prefix) {
            Some(p) => {
                out.insert(key.to_string(), file_info(&p, desc, tail_lines, match_re.as_ref()));
            }
            None => {
                out.insert(
                    key.to_string(),
                    json!({
                        "desc": desc,
                        "path": Value::Null,
                        "note": format!("{} 下无 {prefix}*.log（未产生过该类日志）", to_slash(&dir)),
                    }),
                );
            }
        }
    }
    Ok(json!({ "logs_root": to_slash(&logs_root), "logs": out }))
}

/// 单个日志文件信息
fn file_info(path: &Path, desc: &str, tail_lines: usize, match_re: Option<&regex::Regex>) -> Value {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let to_local = |t: Option<std::time::SystemTime>| -> Value {
        t.map(|t| {
            let secs = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64
                + 8 * 3600;
            let days = secs / 86400;
            let rem = secs % 86400;
            let (y, m, d) = bgd_appsdk::log::civil_from_days(days);
            Value::String(format!(
                "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
                rem / 3600,
                (rem % 3600) / 60,
                rem % 60
            ))
        })
        .unwrap_or(Value::Null)
    };
    let created = meta.as_ref().and_then(|m| m.created().ok());
    let modified = meta.as_ref().and_then(|m| m.modified().ok());

    let lines = count_lines(path).unwrap_or(0);

    let mut info = json!({
        "desc": desc,
        "path": to_slash(path),
        "size": size,
        "created": to_local(created),
        "modified": to_local(modified),
        "lines": lines,
    });
    if match_re.is_some() || tail_lines > 0 {
        scan_log_lines(path, match_re, &mut info);
    }
    if tail_lines > 0 {
        const TAIL_BYTE_CAP: usize = 64 * 1024;
        let (tail, truncated) = read_tail(path, tail_lines, TAIL_BYTE_CAP);
        info["tail"] = Value::String(tail);
        info["truncated"] = Value::Bool(truncated);
    }
    info
}

/// 同条收纳桶（文件日志与 tee 通道共用）：key=去前缀行 → (次数, 最近行号, 最近原文)
struct Bucket {
    order: Vec<String>,
    map: std::collections::HashMap<String, (usize, usize, String)>,
    raw_total: usize,
}
impl Bucket {
    fn new() -> Self {
        Self { order: Vec::new(), map: std::collections::HashMap::new(), raw_total: 0 }
    }
    /// key = 同条比对键（文件行剥 [时间][pid][序号] 前缀；tee 行剥帧号等易变段），line = 展示原文
    fn push(&mut self, lineno: usize, key: String, line: &str) {
        self.raw_total += 1;
        match self.map.get_mut(&key) {
            Some(v) => {
                v.0 += 1;
                v.1 = lineno;
                v.2 = line.to_string();
            }
            None => {
                self.map.insert(key.clone(), (1, lineno, line.to_string()));
                self.order.push(key);
            }
        }
    }
    /// 按「最近出现行号」升序输出（最近发生的排最后），超上限截断保留最新若干条
    fn render(&self, cap: usize) -> (Vec<String>, usize, bool) {
        let mut entries: Vec<&(usize, usize, String)> =
            self.order.iter().filter_map(|k| self.map.get(k)).collect();
        entries.sort_by_key(|v| v.1);
        let distinct = entries.len();
        let truncated = distinct > cap;
        let lines: Vec<String> = entries
            .into_iter()
            .skip(distinct.saturating_sub(cap))
            .map(|(n, lineno, line)| {
                if *n > 1 {
                    format!("{lineno}: {line} (×{n})")
                } else {
                    format!("{lineno}: {line}")
                }
            })
            .collect();
        (lines, distinct, truncated)
    }
}

const MATCH_CAP: usize = 100;
const ERROR_CAP: usize = 20;
const WARN_CAP: usize = 10;

/// 共享级别正则（文件日志与 tee/增量摘要共用）：错误行 / 警告行 / 前缀剥离
fn error_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\[error\]").unwrap())
}
fn warn_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\[(?:warning|warn)\]").unwrap())
}
/// 同条比对键：剥离行首 [时间][pid] 前缀与 [级别] 后的 [序号]（真机日志四组括号
/// [time][pid][level][seq]，seq 逐条递增不去掉会导致刷屏行收纳失败——00_49 真机实测）。
/// 级别括号保留在键内（$1 回填），同文的 info/error 不混淆。
fn prefix_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?x)^\[[^\]]{0,64}\]\[[^\]]{0,16}\]\s*
            ((?i)\[(?:info|error|warning|warn|debug|fatal)\])?(\[\d{1,12}\])?\s*",
        )
        .unwrap()
    })
}

/// 日志行扫描（0.8.0 易用性增强）：
/// - match 命中行：同条收纳（剥离行首 [时间][pid] 前缀后比对，同一条只回最近一次 + ×N 计数），
///   防刷屏行灌爆上下文，同时保留「发生了几次」的排障关键信息；
/// - errors 段：无论 match 是否给出，错误行（[error]/[ERROR]）都单独收纳上浮——
///   前置改动引发的错误会让目标逻辑根本没执行到（假阳性/假阴性），只看 match 命中会漏判。
/// - warns 段（0.8.10 FR-02）：警告行（[warning]/[warn]）同构上浮——warn 级异状
///   （如 protocol duplicate register=探针覆盖游戏 handler）不在 errors 段，曾因此漏看。
/// 行格式："行号: 原文"，重复行附 " (×N)"。
fn scan_log_lines(path: &Path, match_re: Option<&regex::Regex>, info: &mut Value) {
    use std::io::{BufRead, BufReader};

    let Ok(f) = std::fs::File::open(path) else {
        return;
    };

    let mut hits = Bucket::new();
    let mut errors = Bucket::new();
    let mut warns = Bucket::new();
    let mut reader = BufReader::with_capacity(256 * 1024, f);
    let mut buf = Vec::new();
    let mut lineno = 0usize;
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                lineno += 1;
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end();
                if let Some(re) = match_re {
                    if re.is_match(line) {
                        // $1 回填级别括号（保留在键内），时间/pid/序号剥离
                        let key = prefix_re().replace(line, "$1").into_owned();
                        hits.push(lineno, key, line);
                    }
                }
                if error_re().is_match(line) {
                    let key = prefix_re().replace(line, "$1").into_owned();
                    errors.push(lineno, key, line);
                }
                if warn_re().is_match(line) {
                    let key = prefix_re().replace(line, "$1").into_owned();
                    warns.push(lineno, key, line);
                }
            }
            Err(_) => break,
        }
    }

    if let Some(re) = match_re {
        let (lines, distinct, truncated) = hits.render(MATCH_CAP);
        info["match"] = json!({
            "pattern": re.as_str(),
            "total": hits.raw_total,
            "distinct": distinct,
            "returned": lines.len(),
            "truncated": truncated,
            "lines": lines,
        });
    }
    let (lines, distinct, truncated) = errors.render(ERROR_CAP);
    info["errors"] = json!({
        "total": errors.raw_total,
        "distinct": distinct,
        "returned": lines.len(),
        "truncated": truncated,
        "lines": lines,
    });
    let (lines, distinct, truncated) = warns.render(WARN_CAP);
    info["warns"] = json!({
        "total": warns.raw_total,
        "distinct": distinct,
        "returned": lines.len(),
        "truncated": truncated,
        "lines": lines,
    });
}

fn count_lines(path: &Path) -> Option<usize> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::with_capacity(256 * 1024, f);
    let mut n = 0;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => n += 1,
            Err(_) => break,
        }
    }
    Some(n)
}

/// 读文件末尾 N 行（带字节上限；从尾部窗口读避免整文件载入）
fn read_tail(path: &Path, tail_lines: usize, byte_cap: usize) -> (String, bool) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else {
        return (String::new(), false);
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(byte_cap as u64);
    let truncated = start > 0;
    if f.seek(SeekFrom::Start(start)).is_err() {
        return (String::new(), false);
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return (String::new(), false);
    }
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    let lines = if truncated && !lines.is_empty() {
        &lines[1..]
    } else {
        &lines[..]
    };
    let tail: Vec<&str> = lines.iter().rev().take(tail_lines).rev().cloned().collect();
    (tail.join("\n"), truncated)
}

/// 目录下按前缀找最新文件（mtime 优先）
fn latest_file(dir: &Path, prefix: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e == "log").unwrap_or(false)
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with(prefix))
                    .unwrap_or(false)
        })
        .max_by_key(|p| {
            p.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH)
        })
}

// ---------------------------------------------------------------- 0.8.7 分玩家日志（tee 通道）

/// 分玩家日志（在线 tee 通道）：桥 mp_debug.lua 常驻监听调试信息面板的 debug_client_info
/// 投递进每玩家环形缓冲，本函数经桥 mp_logs 拉取后复用文件日志同款 match/errors/同条收纳处理。
/// 边界（与桥侧一致）：tee 无历史（编辑器本次调试注册时点起）、只含面板管线放行的客户端日志；
/// 服务端日志仍走文件型（省略 player）。
pub fn get_game_logs_tee(
    project_root: &Path,
    player: i64,
    tail_lines: usize,
    match_pattern: Option<&str>,
    clear: bool,
    ignore_case: bool,
) -> Result<Value, String> {
    use super::bridge_client;
    let target = locate::locate(project_root)?;
    let port = bridge_client::online_port(&target.engine_root()?)
        .ok_or_else(|| "分玩家日志需编辑器在线（tee 通道）；文件日志请省略 player".to_string())?;
    let res = bridge_client::bridge_rpc(
        port,
        "mp_logs",
        json!({ "player": player, "tail": 1000, "clear": clear }),
        15_000,
    )
    .map_err(|e| format!("{e:#}"))?;
    if clear {
        return Ok(json!({ "cleared": true, "player": player }));
    }

    let match_re = match match_pattern {
        Some(p) if !p.trim().is_empty() => Some(
            regex::RegexBuilder::new(p)
                .case_insensitive(ignore_case)
                .build()
                .map_err(|e| format!("match 正则无效（{e}），请修正后重试"))?,
        ),
        _ => None,
    };

    // tee 行 {player,type,message,frame,location} → 展示行 "[f<帧>][<级别>] <消息>（位置）"；
    // 同条比对键 = 级别+消息（帧号逐帧递增不去掉会收纳失败，与文件行剥序号同理）
    let lines = res
        .get("lines")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total = res.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut hits = Bucket::new();
    let mut errors = Bucket::new();
    let mut warns = Bucket::new();
    let mut display: Vec<String> = Vec::with_capacity(lines.len());
    for (i, l) in lines.iter().enumerate() {
        let ty = l.get("type").and_then(|v| v.as_str()).unwrap_or("信息");
        let msg = l.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let frame = l.get("frame").and_then(|v| v.as_i64());
        let loc = l.get("location").and_then(|v| v.as_str());
        let mut line = match frame {
            Some(f) => format!("[f{f}][{ty}] {msg}"),
            None => format!("[{ty}] {msg}"),
        };
        if let Some(loc) = loc.filter(|s| !s.is_empty()) {
            line.push_str(&format!("（{loc}）"));
        }
        let key = format!("{ty}|{msg}");
        if let Some(re) = &match_re {
            if re.is_match(&line) {
                hits.push(i + 1, key.clone(), &line);
            }
        }
        // 面板管线已把级别映射为中文（错误/信息/警告），errors/warns 段按级别上浮（0.8.10 FR-02）
        if ty == "错误" {
            errors.push(i + 1, key.clone(), &line);
        }
        if ty == "警告" {
            warns.push(i + 1, key, &line);
        }
        display.push(line);
    }

    let mut info = json!({
        "channel": "tee",
        "player": player,
        "total": total,
        "note": res.get("note").cloned().unwrap_or(Value::Null),
    });
    if let Some(re) = &match_re {
        let (ls, distinct, truncated) = hits.render(MATCH_CAP);
        info["match"] = json!({
            "pattern": re.as_str(),
            "total": hits.raw_total,
            "distinct": distinct,
            "returned": ls.len(),
            "truncated": truncated,
            "lines": ls,
        });
    }
    let (ls, distinct, truncated) = errors.render(ERROR_CAP);
    info["errors"] = json!({
        "total": errors.raw_total,
        "distinct": distinct,
        "returned": ls.len(),
        "truncated": truncated,
        "lines": ls,
    });
    let (ls, distinct, truncated) = warns.render(WARN_CAP);
    info["warns"] = json!({
        "total": warns.raw_total,
        "distinct": distinct,
        "returned": ls.len(),
        "truncated": truncated,
        "lines": ls,
    });
    if tail_lines > 0 {
        let tail: Vec<&String> = display.iter().rev().take(tail_lines).rev().collect();
        info["tail"] = Value::String(
            tail.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        );
    }
    Ok(info)
}

// ---------------------------------------------------------------- 0.8.10 场景日志增量摘要（R2）

/// 场景日志快照：记录 game_client/game_server 最新文件路径与行数，供场景结束时对比
/// 「期间新增了什么」。离线可用、永不失败（定位失败回空表，摘要侧自然降级）。
pub fn snapshot_game_logs(project_root: &Path) -> Value {
    let mut out = serde_json::Map::new();
    if let Ok(target) = locate::locate(project_root) {
        if let Ok(root) = target.engine_root() {
            let logs_root = root.join("logs");
            for (key, sub, prefix, _) in LOG_SOURCES
                .iter()
                .filter(|(k, ..)| *k == "game_client" || *k == "game_server")
            {
                if let Some(p) = latest_file(&logs_root.join(sub), prefix) {
                    out.insert(
                        key.to_string(),
                        json!({ "path": to_slash(&p), "lines": count_lines(&p).unwrap_or(0) }),
                    );
                }
            }
        }
    }
    Value::Object(out)
}

/// 场景日志增量摘要：对比快照统计期间新增行数 / errors / warnings / top 重复行
/// （同条收纳取次数最高者）——刷屏类问题（BUG-04 型：非 error 级，errors 段看不见）
/// 靠 top_repeats 天然暴露。文件轮换（路径变了）按全文件统计。
pub fn summarize_log_delta(project_root: &Path, start: &Value) -> Value {
    use std::io::{BufRead, BufReader};
    let mut out = serde_json::Map::new();
    let Ok(target) = locate::locate(project_root) else {
        return json!({ "note": "项目定位失败，无法生成日志摘要" });
    };
    let Ok(root) = target.engine_root() else {
        return json!({ "note": "引擎根定位失败，无法生成日志摘要" });
    };
    let logs_root = root.join("logs");
    for (key, sub, prefix, desc) in LOG_SOURCES
        .iter()
        .filter(|(k, ..)| *k == "game_client" || *k == "game_server")
    {
        let Some(p) = latest_file(&logs_root.join(sub), prefix) else {
            continue;
        };
        let path = to_slash(&p);
        // 同一文件 → 跳过快照时点前的旧行；文件已轮换 → 从 0 计
        let skip = start
            .get(key)
            .filter(|s| s.get("path").and_then(|v| v.as_str()) == Some(path.as_str()))
            .and_then(|s| s.get("lines"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let mut new_lines = 0usize;
        let mut errors = Bucket::new();
        let mut warns = Bucket::new();
        let mut repeats = Bucket::new();
        if let Ok(f) = std::fs::File::open(&p) {
            let mut reader = BufReader::with_capacity(256 * 1024, f);
            let mut buf = Vec::new();
            let mut lineno = 0usize;
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        lineno += 1;
                        if lineno <= skip {
                            continue;
                        }
                        new_lines += 1;
                        let line = String::from_utf8_lossy(&buf);
                        let line = line.trim_end();
                        let key = prefix_re().replace(line, "$1").into_owned();
                        if error_re().is_match(line) {
                            errors.push(lineno, key.clone(), line);
                        }
                        if warn_re().is_match(line) {
                            warns.push(lineno, key.clone(), line);
                        }
                        repeats.push(lineno, key, line);
                    }
                    Err(_) => break,
                }
            }
        }
        // top 重复行：按次数降序取前 3（仅重复 >1 次的；BUG-04 型刷屏在这里现形）
        let mut tops: Vec<(&String, &(usize, usize, String))> = repeats.map.iter().collect();
        tops.sort_by_key(|(_, (n, _, _))| std::cmp::Reverse(*n));
        let top_repeats: Vec<Value> = tops
            .into_iter()
            .filter(|(_, (n, _, _))| *n > 1)
            .take(3)
            .map(|(_, (n, _, line))| json!({ "count": n, "line": line }))
            .collect();
        let (err_lines, _, _) = errors.render(5);
        let (warn_lines, _, _) = warns.render(5);
        out.insert(
            key.to_string(),
            json!({
                "desc": desc,
                "path": path,
                "new_lines": new_lines,
                "errors": errors.raw_total,
                "error_lines": err_lines,
                "warnings": warns.raw_total,
                "warn_lines": warn_lines,
                "top_repeats": top_repeats,
            }),
        );
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_from_days() {
        // 算法单一真相在 bgd_appsdk::log（此处守护调用约定的换算结果）
        assert_eq!(bgd_appsdk::log::civil_from_days(0), (1970, 1, 1));
        assert_eq!(bgd_appsdk::log::civil_from_days(20270), (2025, 7, 1));
    }

    #[test]
    fn test_tail_and_count() {
        let dir = std::env::temp_dir().join(format!("bgd_logs_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("lua-game-20260101.log");
        let content: String = (1..=100).map(|i| format!("line{i}\n")).collect();
        std::fs::write(&f, &content).unwrap();
        assert_eq!(count_lines(&f), Some(100));
        let (tail, truncated) = read_tail(&f, 3, 64 * 1024);
        assert!(!truncated);
        assert_eq!(tail, "line98\nline99\nline100");
        let (tail2, truncated2) = read_tail(&f, 3, 64);
        assert!(truncated2);
        assert!(tail2.contains("line100"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_latest_file() {
        let dir = std::env::temp_dir().join(format!("bgd_logs_latest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lua-game-1.log"), "a").unwrap();
        std::fs::write(dir.join("core-game-server-1.log"), "b").unwrap();
        std::fs::write(dir.join("other.txt"), "c").unwrap();
        let got = latest_file(&dir, "lua-game-").unwrap();
        assert!(got.ends_with("lua-game-1.log"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_match_filter() {
        let dir = std::env::temp_dir().join(format!("bgd_logs_match_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("lua-game-20260101.log");
        // 10 条普通 + 同一错误刷屏 3 次（前缀时间/pid/序号不同）+ 另一条错误 1 次
        let mut content = String::new();
        for i in 1..=10 {
            content.push_str(&format!("[2026-08-29 01:02:{i:02}.000][8968][info][100][a.lua:1] line{i} ok\n"));
        }
        for i in 20..=22 {
            content.push_str(&format!(
                "[2026-08-29 01:03:{i}.844][8968][error][{seq}][b.lua:2] [ERROR] [c.lua:3] [cgui] 组合函数异常（hub）\n",
                seq = 68949 + i
            ));
        }
        content.push_str("[2026-08-29 01:04:00.000][8968][error][69000][d.lua:4] [ERROR] 另一错误\n");
        std::fs::write(&f, &content).unwrap();

        // match 命中普通行：同条收纳生效（互不相同的行各自一条）
        let re = regex::Regex::new("ok$").unwrap();
        let info = file_info(&f, "d", 0, Some(&re));
        let m = &info["match"];
        assert_eq!(m["total"].as_u64(), Some(10));
        assert_eq!(m["distinct"].as_u64(), Some(10));
        assert_eq!(m["returned"].as_u64(), Some(10));

        // match 命中刷屏错误：收纳为 1 条 + ×3 计数（取最近一次行号）
        let re2 = regex::Regex::new("组合函数异常").unwrap();
        let info2 = file_info(&f, "d", 0, Some(&re2));
        let m2 = &info2["match"];
        assert_eq!(m2["total"].as_u64(), Some(3));
        assert_eq!(m2["distinct"].as_u64(), Some(1));
        let lines = m2["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].as_str().unwrap().starts_with("13: "));
        assert!(lines[0].as_str().unwrap().ends_with("(×3)"));

        // errors 段：无论 match 与否都上浮；两条不同错误各一次收纳
        let e2 = &info2["errors"];
        assert_eq!(e2["total"].as_u64(), Some(4));
        assert_eq!(e2["distinct"].as_u64(), Some(2));
        let elines = e2["lines"].as_array().unwrap();
        assert_eq!(elines.len(), 2);
        assert!(elines[0].as_str().unwrap().contains("(×3)"));
        assert!(!elines[1].as_str().unwrap().contains('×'));

        // tail 与 match 可同用；tail 模式也带 errors 段
        let info3 = file_info(&f, "d", 3, Some(&re));
        assert!(info3["tail"].as_str().unwrap().contains("另一错误"));
        assert!(info3["errors"]["total"].as_u64() == Some(4));
        let info4 = file_info(&f, "d", 3, None);
        assert!(info4["errors"]["distinct"].as_u64() == Some(2));
        assert!(info4.get("match").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真机冒烟：对真实日志文件跑 match + errors 收纳（BGD_SMOKE_LOG 环境变量指定路径）
    #[test]
    #[ignore = "真机冒烟：cargo test -- --ignored 且设 BGD_SMOKE_LOG=<日志文件路径>"]
    fn test_smoke_real_log() {
        let p = std::env::var("BGD_SMOKE_LOG").expect("先设 BGD_SMOKE_LOG=<日志文件路径>");
        let re = regex::Regex::new("组合函数异常").unwrap();
        let info = file_info(std::path::Path::new(&p), "smoke", 5, Some(&re));
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    }
}
