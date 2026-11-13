//! 联想拿哪句话（`memory.md` 第四条第 1 款）：人开的一轮交触发的那句；人生循环叫醒的（施工 LF-3）、她自己定节奏的循环叫醒的
//! 都没有，醒来那段话每次都一样，拿它联想只会带回噪声。

use super::*;
use crate::id::Seq;

/// 第 `seq` 条：`by` 说了一句。
fn message(seq: u64, by: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-11-14T07:00:{seq:02}.000Z","kind":"message.user","by":{by},"body":{{"blocks":[{{"type":"text","text":"第 {seq} 句"}}]}}}}"#
    ))
    .unwrap()
}

/// 第 `seq` 条：照 `trigger` 开一轮。
fn started(seq: u64, trigger: u64) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-11-14T07:00:{seq:02}.000Z","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{{"trigger":{trigger}}}}}"#
    ))
    .unwrap()
}

#[test]
fn a_turn_the_life_loop_woke_has_nothing_to_recall_with() {
    let mut history = History::whole();
    for event in [
        message(1, r#"{"kind":"person"}"#),
        started(2, 1),
        message(3, r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#),
        started(4, 3),
        message(5, r#"{"kind":"schedule","schedule":"t2","loop":"dynamic"}"#),
        started(6, 5),
    ] {
        history.append(event);
    }
    let turn = |seq: u64| TurnId::new(Seq::new(seq).unwrap());
    assert_eq!(said(&history, turn(2)).as_deref(), Some("第 1 句"));
    assert_eq!(said(&history, turn(4)), None, "人生循环叫醒的不联想");
    assert_eq!(
        said(&history, turn(6)),
        None,
        "她自己定节奏的循环叫醒的不联想"
    );
}
