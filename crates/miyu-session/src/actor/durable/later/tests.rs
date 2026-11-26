//! 带工具调用的那一批晚一点同步（施工 BO-4）：哪一批等、什么时候叫起来。事件照真日志里的样子写。

use std::time::Duration;

use miyu_kernel::event::Event;

use super::{Later, Seen};

fn event(line: &str) -> Event {
    Event::from_line(line).expect("事件读得出")
}

/// 一条带 `calls` 个工具调用的回复。
fn reply(calls: usize) -> Event {
    let blocks: Vec<String> = (1..=calls)
        .map(|n| {
            format!(r#"{{"type":"tool_call","call_id":"call_9_{n}","name":"read","args":"{{}}"}}"#)
        })
        .collect();
    event(&format!(
        r#"{{"seq":8,"at":"2026-10-24T14:27:28.028Z","kind":"message.assistant","turn":3,"by":{{"kind":"model","endpoint":"bench","model":"m"}},"body":{{"blocks":[{}],"seen":7}}}}"#,
        blocks.join(",")
    ))
}

/// 第 `n` 个调用（从 0 数）的结果。
fn result(n: usize) -> Event {
    let n = n + 1;
    event(&format!(
        r#"{{"seq":10,"at":"2026-10-24T14:27:28.043Z","kind":"tool.result","turn":3,"by":{{"kind":"tool","call_id":"call_9_{n}"}},"body":{{"call_id":"call_9_{n}","status":"ok","blocks":[{{"type":"text","text":"ok"}}]}}}}"#
    ))
}

fn said() -> Event {
    event(
        r#"{"seq":2,"at":"2026-10-24T14:27:25.766Z","kind":"message.user","by":{"kind":"person"},"cause":"c","body":{"blocks":[{"type":"text","text":"你好"}]}}"#,
    )
}

fn turn_ended() -> Event {
    event(
        r#"{"seq":12,"at":"2026-10-24T14:27:28.050Z","kind":"turn.ended","turn":3,"by":{"kind":"kernel"},"body":{"reason":"completed"}}"#,
    )
}

/// 等着的那一头这时被叫了没有。
async fn called(go: &tokio::sync::Notify) -> bool {
    tokio::time::timeout(Duration::from_millis(20), go.notified())
        .await
        .is_ok()
}

#[test]
fn a_batch_is_read_for_its_calls_results_and_whether_it_may_wait() {
    let seen = Seen::of(&[reply(2)]);
    assert_eq!((seen.calls, seen.results, seen.patient), (2, 0, true));
    let seen = Seen::of(&[result(0), result(1)]);
    assert_eq!((seen.calls, seen.results, seen.patient), (0, 2, true));
    assert!(!Seen::of(&[said(), reply(1)]).patient, "外面交来的不等");
    assert!(
        !Seen::of(&[reply(1), turn_ended()]).patient,
        "一轮收尾的不等"
    );
}

#[tokio::test]
async fn a_reply_with_tool_calls_waits_until_every_result_is_written() {
    let mut later = Later::default();
    let go = later
        .written(Seen::of(&[reply(2)]), true, true)
        .expect("带工具调用的回复先等");
    assert!(
        later
            .written(Seen::of(&[result(0)]), false, false)
            .is_none()
    );
    assert!(!called(&go).await, "还有一个调用没出结果，接着等");
    assert!(
        later
            .written(Seen::of(&[result(1)]), false, false)
            .is_none()
    );
    assert!(called(&go).await, "结果都写进去了，现在就同步");
}

#[tokio::test]
async fn a_batch_that_must_be_synced_wakes_the_waiting_sync() {
    let mut later = Later::default();
    let go = later
        .written(Seen::of(&[reply(1)]), true, true)
        .expect("先等");
    // 工具还在跑，人插了一句话：这一批非同步不可，不再等工具。
    assert!(later.written(Seen::of(&[said()]), true, false).is_none());
    assert!(called(&go).await);
}

#[tokio::test]
async fn a_held_reply_or_a_stop_wakes_the_waiting_sync() {
    let mut later = Later::default();
    let go = later
        .written(Seen::of(&[reply(1)]), true, true)
        .expect("先等");
    later.go();
    assert!(called(&go).await);
    // 没有在等的再叫一次什么都不做。
    later.go();
}

#[test]
fn batches_that_touch_the_outside_or_end_a_turn_never_wait() {
    let mut later = Later::default();
    // 人的话、一轮收尾：马上同步。
    assert!(later.written(Seen::of(&[said()]), true, true).is_none());
    assert!(
        later
            .written(Seen::of(&[turn_ended()]), true, true)
            .is_none()
    );
    // 带工具调用的回复和人的话在同一批里：也不等。
    assert!(
        later
            .written(Seen::of(&[said(), reply(1)]), true, true)
            .is_none()
    );
    // 回复和它的结果在同一批里（调用都出了结果）：没有要等的。
    assert!(
        later
            .written(Seen::of(&[reply(1), result(0)]), true, true)
            .is_none()
    );
    // 这一批没有当场派同步（已经有一次在路上）：不等。
    assert!(later.written(Seen::of(&[reply(1)]), true, false).is_none());
}

#[tokio::test]
async fn an_interrupted_reply_does_not_keep_the_next_one_waiting() {
    let mut later = Later::default();
    // 上一条回复的三个调用只出了一个结果就被打断了。
    let old = later
        .written(Seen::of(&[reply(3)]), true, true)
        .expect("先等");
    assert!(
        later
            .written(Seen::of(&[result(0)]), false, false)
            .is_none()
    );
    assert!(!called(&old).await);
    // 新的一条回复只数它自己的调用。
    let go = later
        .written(Seen::of(&[reply(1)]), true, true)
        .expect("先等");
    assert!(
        later
            .written(Seen::of(&[result(0)]), false, false)
            .is_none()
    );
    assert!(called(&go).await);
}
