//! 封存（施工 C-8 下）：`session.sealed` 只在两轮之间、只记一次；封存以后不再开一轮，别的照记。

use super::*;

/// 合规的会话前 9 条（第一轮结束）再封存。
fn sealed() -> Ledger {
    let mut ledger = after(9);
    ledger
        .append(&event(10, None, "session.sealed", "{}"))
        .unwrap();
    ledger
}

#[test]
fn a_session_is_sealed_once_between_turns() {
    let mut ledger = after(9);
    assert!(!ledger.sealed());
    ledger
        .append(&event(10, None, "session.sealed", "{}"))
        .unwrap();
    assert!(ledger.sealed());
    refused(
        &mut ledger,
        &event(11, None, "session.sealed", "{}"),
        "already sealed",
    );
    // 在一轮当中封存的拒。
    let mut running = after(4);
    refused(
        &mut running,
        &event(5, None, "session.sealed", "{}"),
        "still running",
    );
}

#[test]
fn a_sealed_session_starts_no_turn_but_still_records() {
    let mut ledger = sealed();
    ledger
        .append(&event(11, None, "message.user", SAID))
        .unwrap();
    refused(
        &mut ledger,
        &event(12, Some(12), "turn.started", r#"{"trigger":11}"#),
        "starts no turn",
    );
}
