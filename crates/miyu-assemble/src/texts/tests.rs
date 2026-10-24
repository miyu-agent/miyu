//! 回合结束的原因说哪一句（`TurnEndedTexts::for_reason`）。

use super::*;

fn texts() -> TurnEndedTexts {
    TurnEndedTexts {
        interrupted: "<interrupted/>".to_string(),
        error: "<error/>".to_string(),
        step_limit: "<step-limit/>".to_string(),
        aborted: "<aborted/>".to_string(),
        restarted: "<restarted/>".to_string(),
    }
}

/// fork 子代理收住接过来的那一轮（施工 A-F 上）：不是没走完，同走完了的一样什么都不说。
#[test]
fn a_turn_settled_by_a_fork_says_nothing() {
    let texts = texts();
    assert_eq!(texts.for_reason(&EndReason::Forked), None);
    assert_eq!(texts.for_reason(&EndReason::Completed), None);
    assert_eq!(
        texts.for_reason(&EndReason::Interrupted),
        Some("<interrupted/>")
    );
}
