//! 取证覆盖率自检（0.8.10 R3）：MCP 进程级会话计数器。
//!
//! 计数三类动作：captures（截图次数）/ ui_ops（真实 UI 操作族调用次数）/ eval_ops（lua.eval
//! 逃生舱调用次数）。get_status 与 run_scenario 结束附 evidence 段；当 eval_ops 远大于
//! ui_ops 且 captures == 0 时附失衡 hint——「画面没看过」是可检测状态，不依赖用户旁观发现。
//!
//! 与 skills 的分工：工序 skill 管「该怎么做」（完成定义四条），计数器管「没做对时提醒」，
//! 两层不冲突。计数器是兜底自检，成本极低（三个原子计数器）。

use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static CAPTURES: AtomicU64 = AtomicU64::new(0);
static UI_OPS: AtomicU64 = AtomicU64::new(0);
static EVAL_OPS: AtomicU64 = AtomicU64::new(0);

/// UI 操作族能力 id（真实链路验证计数；find_ui/game_info 等只读观察不计）
const UI_OP_IDS: &[&str] = &[
    "lua.tap",
    "lua.pick",
    "lua.click_ui",
    "lua.click_at",
    "lua.press_ui",
    "lua.release_ui",
    "lua.long_press_ui",
    "lua.hover_ui",
    "lua.drag_ui",
    "lua.scroll_ui",
    "lua.key_down",
    "lua.key_up",
    "lua.input_text",
    "lua.set_value",
];

/// 截图计数 +1（capture_game 工具 / 场景 capture 系步骤 / snapshot 视觉回执统一漏斗）
pub fn count_capture() {
    CAPTURES.fetch_add(1, Ordering::Relaxed);
}

/// invoke 类调用分类计数：lua.eval/lua.server_eval → eval_ops；UI 操作族 → ui_ops；其余不计
pub fn count_invoke(id: &str) {
    if id == "lua.eval" || id == "lua.server_eval" {
        EVAL_OPS.fetch_add(1, Ordering::Relaxed);
    } else if UI_OP_IDS.contains(&id) {
        UI_OPS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn snapshot() -> Value {
    json!({
        "captures": CAPTURES.load(Ordering::Relaxed),
        "ui_ops": UI_OPS.load(Ordering::Relaxed),
        "eval_ops": EVAL_OPS.load(Ordering::Relaxed),
    })
}

/// 失衡检测：大量 eval + 零截图 → 画面取证缺失提醒（阈值：eval≥5 且超 ui_ops 两倍）
pub fn imbalance_hint() -> Option<&'static str> {
    let c = CAPTURES.load(Ordering::Relaxed);
    let u = UI_OPS.load(Ordering::Relaxed);
    let e = EVAL_OPS.load(Ordering::Relaxed);
    if c == 0 && e >= 5 && e > u.saturating_mul(2) {
        Some(
            "本会话大量 eval 但零截图——若测的是 UI 功能，画面取证缺失：UI 操作走 lua.tap 等真实链路，capture_game/capture_ui 补截图（读图走视觉通道）",
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试专用：强制设定计数（计数器进程级共享，测试需确定性初值）
    fn force_set(c: u64, u: u64, e: u64) {
        CAPTURES.store(c, Ordering::Relaxed);
        UI_OPS.store(u, Ordering::Relaxed);
        EVAL_OPS.store(e, Ordering::Relaxed);
    }

    #[test]
    fn test_imbalance_threshold() {
        // 未达阈值：eval<5 / 有截图 / eval 占比不高 —— 均不提醒
        force_set(0, 0, 4);
        assert!(imbalance_hint().is_none());
        force_set(1, 0, 10);
        assert!(imbalance_hint().is_none());
        force_set(0, 4, 8);
        assert!(imbalance_hint().is_none());
        // 失衡：eval≥5 且 > ui_ops*2 且零截图 → 提醒
        force_set(0, 2, 5);
        assert!(imbalance_hint().is_some());
        force_set(0, 0, 5);
        assert!(imbalance_hint().is_some());
        // 复位，避免影响同进程其他测试
        force_set(0, 0, 0);
    }

    #[test]
    fn test_count_classification() {
        force_set(0, 0, 0);
        count_invoke("lua.eval");
        count_invoke("lua.tap");
        count_invoke("lua.find_ui"); // 只读观察不计
        count_invoke("svc.Foo.Bar"); // 非 lua 操作族不计
        count_capture();
        let s = snapshot();
        assert_eq!(s["eval_ops"], json!(1));
        assert_eq!(s["ui_ops"], json!(1));
        assert_eq!(s["captures"], json!(1));
        force_set(0, 0, 0);
    }
}
