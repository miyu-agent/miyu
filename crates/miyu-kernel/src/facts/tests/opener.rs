//! 开这一轮的那条消息是谁发的（施工 LF-6，`memory.md` 第三条第 9 款）：记忆照它认出人生循环叫醒的一轮。人、人生循环、她自己
//! 定节奏的循环都原样交；没有 `trigger` 的、触发的不是消息的没有。

use super::*;
use crate::id::Seq;
use crate::origin::{Schedule, ScheduleLoop};

/// 第 `seq` 条：`by` 说了一句。
fn message(seq: u64, by: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-11-14T07:00:{seq:02}.000Z","kind":"message.user","by":{by},"body":{{"blocks":[{{"type":"text","text":"第 {seq} 句"}}]}}}}"#
    ))
    .unwrap()
}

/// 第 `seq` 条：照 `trigger` 开一轮；没有的是人要的压缩单开的那种。
fn started(seq: u64, trigger: Option<u64>) -> Event {
    let body = trigger.map_or("{}".to_string(), |trigger| {
        format!(r#"{{"trigger":{trigger}}}"#)
    });
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-11-14T07:00:{seq:02}.000Z","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .unwrap()
}

fn turn(seq: u64) -> TurnId {
    TurnId::new(Seq::new(seq).unwrap())
}

fn schedule(looping: Option<ScheduleLoop>) -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping,
    })
}

#[test]
fn the_opener_is_whoever_sent_the_trigger() {
    let mut history = History::whole();
    for event in [
        message(1, r#"{"kind":"person"}"#),
        started(2, Some(1)),
        message(3, r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#),
        started(4, Some(3)),
        message(5, r#"{"kind":"schedule","schedule":"t1","loop":"dynamic"}"#),
        started(6, Some(5)),
        message(7, r#"{"kind":"schedule","schedule":"t1"}"#),
        started(8, Some(7)),
        started(9, None),
    ] {
        history.append(event);
    }
    assert!(matches!(opener(&history, turn(2)), Some(By::Person(_))));
    assert_eq!(
        opener(&history, turn(4)),
        Some(schedule(Some(ScheduleLoop::Life))),
        "人生循环叫醒的"
    );
    assert_eq!(
        opener(&history, turn(6)),
        Some(schedule(Some(ScheduleLoop::Dynamic)))
    );
    assert_eq!(opener(&history, turn(8)), Some(schedule(None)), "定时任务");
    assert_eq!(opener(&history, turn(9)), None, "没有 trigger 的没有");
    assert_eq!(opener(&history, turn(1)), None, "不是一轮的没有");
}

#[test]
fn a_trigger_that_is_not_a_message_has_no_opener() {
    let mut history = History::whole();
    for event in [
        Event::from_line(
            r#"{"seq":1,"at":"2026-11-14T07:00:01.000Z","kind":"context.injected","by":{"kind":"module","id":"memory"},"body":{"kind":"memory","text":"x"}}"#,
        )
        .unwrap(),
        started(2, Some(1)),
    ] {
        history.append(event);
    }
    assert_eq!(opener(&history, turn(2)), None);
}
