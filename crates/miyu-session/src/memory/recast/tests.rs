//! 换人格的分界（施工 P-13）：最近一条换成这个人格的 `policy.changed` 是界；换成别的、只换了模型的不算；空的是无人格。
//! 界以前的（连同它）并进底子，底子原来有的在前面。

use miyu_kernel::event::Event;

use super::*;

/// 一条事件：序号、种类、`body`。
fn event(seq: u64, kind: &str, body: &str) -> Event {
    serde_json::from_str(&format!(
        r#"{{"seq":{seq},"at":"2026-10-19T01:00:{seq:02}.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .expect("合写法")
}

/// 一份日志：2 换成 b，3 只换模型，5 换回 a，7 换成无人格；别的是人说的话。
fn log() -> Vec<Event> {
    (1..=8)
        .map(|seq| match seq {
            2 => event(seq, "session.policy_changed", r#"{"persona":"b"}"#),
            3 => event(seq, "session.policy_changed", r#"{"model":"p/m"}"#),
            5 => event(seq, "session.policy_changed", r#"{"persona":"a"}"#),
            7 => event(seq, "session.policy_changed", r#"{"persona":""}"#),
            _ => event(seq, "message.user", r#"{"blocks":[]}"#),
        })
        .collect()
}

#[test]
fn the_floor_is_the_last_change_to_that_persona() {
    let log = log();
    let at = |persona| floor(&log, persona).map(Seq::get);
    assert_eq!(at("b"), Some(2));
    assert_eq!(at("a"), Some(5), "换回来的照最近那一条");
    assert_eq!(at(NO_PERSONA), Some(7), "空的是无人格");
    assert_eq!(at("c"), None, "没换成过的没有");
    assert_eq!(floor(&log[..1], "b"), None);
}

#[test]
fn splitting_moves_everything_up_to_the_floor_into_the_base() {
    let log = log();
    let base = vec![event(1, "message.user", r#"{"blocks":[]}"#)];
    let (joined, own) = split(&base, &log, Some(Seq::new(5).expect("非零")));
    let seqs = |events: &[Event]| {
        events
            .iter()
            .map(|event| event.seq.get())
            .collect::<Vec<_>>()
    };
    assert_eq!(seqs(&joined), [1, 1, 2, 3, 4, 5], "底子原来的在前面");
    assert_eq!(seqs(own), [6, 7, 8]);
    let (kept, all) = split(&base, &log, None);
    assert_eq!((seqs(&kept), all.len()), (vec![1], 8), "没有界的照旧");
}
