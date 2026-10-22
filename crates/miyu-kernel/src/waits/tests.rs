//! 等人不算用时的测试：一件、几件一起等算一段；还在等的算到哪一刻；回合开始、结束清掉；一个会话干活的时间跨轮累加，在跑的
//! 那一轮算到此刻。

use super::*;

/// 第 `s` 秒（同一分钟里）。
fn at(s: i64) -> Timestamp {
    Timestamp::from_unix_millis(1_790_000_000_000 + s * 1000).unwrap()
}

fn call(n: u32) -> CallId {
    CallId::parse(&format!("call_5_{n}")).unwrap()
}

/// 第 `s` 秒的一条 `kind`，正文 `body`。
fn event(seq: u64, s: i64, kind: &str, body: &str) -> Event {
    let at = at(s).to_string();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"{at}","kind":"{kind}","turn":1,"by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .unwrap()
}

#[test]
fn one_wait_is_cut_out_of_the_time() {
    let mut waits = Waits::default();
    waits.wait(&call(1), at(10));
    assert_eq!(waits.waiting_since(), Some(at(10)));
    waits.settled(&call(1), at(25));
    assert_eq!(waits.waiting_since(), None);
    assert_eq!(waits.waited_ms(at(0), at(60)), 15_000);
    assert_eq!(waits.busy_ms(at(0), at(60)), 45_000);
}

#[test]
fn waits_that_overlap_count_once_and_end_with_the_last() {
    let mut waits = Waits::default();
    waits.wait(&call(1), at(10));
    waits.wait(&call(2), at(12));
    waits.wait(&call(1), at(13));
    waits.settled(&call(1), at(20));
    assert_eq!(waits.waiting_since(), Some(at(10)), "还有一件在等");
    waits.settled(&call(2), at(30));
    assert_eq!(waits.waited_ms(at(0), at(60)), 20_000);
    // 没在等的了结不起一段。
    waits.settled(&call(3), at(40));
    assert_eq!(waits.waited_ms(at(0), at(60)), 20_000);
}

#[test]
fn an_open_wait_counts_up_to_the_end_asked_for() {
    let mut waits = Waits::default();
    waits.wait(&call(1), at(10));
    assert_eq!(waits.busy_ms(at(0), at(30)), 10_000);
    // 只算区间以内的那一截。
    assert_eq!(waits.waited_ms(at(20), at(30)), 10_000);
    assert_eq!(waits.waited_ms(at(0), at(5)), 0);
}

#[test]
fn a_turn_boundary_clears_the_waits() {
    let mut waits = Waits::default();
    waits.wait(&call(1), at(10));
    assert!(waits.note(&event(5, 20, "turn.ended", r#"{"reason":"interrupted"}"#)));
    assert_eq!(waits.waiting_since(), None);
    assert_eq!(waits.waited_ms(at(0), at(60)), 0);
    assert!(!waits.note(&event(6, 21, "session.sealed", "{}")));
    // 被停、被拒的调用也有结果：结果到了就不再等。
    waits.wait(&call(1), at(30));
    assert!(waits.note(&event(
        7,
        35,
        "tool.result",
        r#"{"call_id":"call_5_1","status":"ok","blocks":[]}"#
    )));
    assert_eq!(waits.waiting_since(), None);
    assert_eq!(waits.waited_ms(at(0), at(60)), 5_000);
}

#[test]
fn busy_adds_up_the_turns_without_the_waits() {
    let mut busy = Busy::default();
    busy.note(&event(1, 0, "turn.started", "{}"));
    busy.waits.wait(&call(1), at(10));
    busy.waits.settled(&call(1), at(40));
    busy.note(&event(2, 60, "turn.ended", r#"{"reason":"completed"}"#));
    assert_eq!(busy.total_ms(at(100)), 30_000, "一轮 60 秒，等了 30 秒");
    // 轮和轮中间闲着的不算；在跑的那一轮算到此刻。
    busy.note(&event(3, 100, "turn.started", "{}"));
    assert_eq!(busy.total_ms(at(110)), 40_000);
    busy.waits.wait(&call(2), at(110));
    assert_eq!(busy.total_ms(at(130)), 40_000, "正在等的不算");
}
