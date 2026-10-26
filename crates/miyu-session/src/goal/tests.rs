//! 长期目标照日志算（施工 X-6）：设、改、停、接着跑、卡住、清掉一路的样子；停着时开的不算轮数、不算花的；结束了的照最后的
//! 样子留着，再设一个换掉；一批批加上的和从头算的一样。

use miyu_kernel::event::Event;

use super::*;

/// 一条事件：序号、种类、`body`，时刻照序号往后走。
fn event(seq: u64, kind: &str, body: &str) -> Event {
    serde_json::from_str(&format!(
        r#"{{"seq":{seq},"at":"2026-10-12T01:00:{seq:02}.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .expect("合写法")
}

/// 开一轮。
fn started(seq: u64) -> Event {
    event(
        seq,
        "turn.started",
        &format!(r#"{{"trigger":{}}}"#, seq - 1),
    )
}

/// 一次发出去的主请求，输出 `output` 个。
fn called(seq: u64, output: u64) -> Event {
    event(
        seq,
        "model.called",
        &format!(
            r#"{{"seen":{},"messages":2,"result":"ok","endpoint":"dev","model":"m","request":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","usage":{{"uncached":10,"cache_read":0,"cache_write":0,"output":{output}}}}}"#,
            seq - 1
        ),
    )
}

fn story() -> Vec<Event> {
    vec![
        started(2),
        called(3, 100),
        event(4, "goal.set", r#"{"text":"修绿","limit":5}"#),
        started(5),
        called(6, 7),
        event(7, "goal.changed", r#"{"text":"修绿再写文档"}"#),
        started(8),
        called(9, 8),
        event(10, "goal.paused", "{}"),
        started(11),
        called(12, 1000),
        event(13, "goal.resumed", "{}"),
        started(14),
        called(15, 9),
        event(
            16,
            "goal.ended",
            r#"{"outcome":"blocked","reason":"要人给 key"}"#,
        ),
        started(17),
        called(18, 50),
    ]
}

#[test]
fn a_goal_follows_the_log_from_set_to_blocked() {
    let goals = Goals::of(&story());
    let goal = goals.now().expect("卡住的照最后的样子留着");
    assert_eq!(goal.text, "修绿再写文档", "改过的照改完的");
    assert_eq!(goal.limit, None, "改的时候没写上限，就是不限了");
    assert_eq!(goal.state, GoalState::Blocked);
    assert_eq!(goal.reason.as_deref(), Some("要人给 key"));
    assert_eq!(goal.round, 3, "设下以前的、停着时的、结束以后的都不算");
    assert_eq!(goal.resumes, 1, "停过一次又接着跑");
    assert_eq!(goal.spent.usage.output, 7 + 8 + 9, "只算跑着时的请求");
    assert_eq!(goal.since.to_string(), "2026-10-12T01:00:04.000Z");
}

#[test]
fn batches_add_up_to_the_same_as_from_the_start() {
    let events = story();
    let mut split = Goals::of(&events[..6]);
    split.add(&events[6..11]);
    split.add(&events[11..]);
    assert_eq!(split, Goals::of(&events));
}

#[test]
fn clearing_forgets_the_goal_and_a_new_one_starts_afresh() {
    let mut events = story();
    events.push(event(19, "goal.set", r#"{"text":"下一个"}"#));
    let goals = Goals::of(&events);
    let goal = goals.now().expect("新设的");
    assert_eq!(
        (goal.text.as_str(), goal.round, goal.resumes, goal.state),
        ("下一个", 0, 0, GoalState::Running)
    );
    assert_eq!(goal.spent.usage.output, 0, "从头数");
    events.push(event(20, "goal.ended", r#"{"outcome":"cleared"}"#));
    assert_eq!(Goals::of(&events).now(), None, "清掉的不再给人看");
    assert_eq!(Goals::of(&events[..2]).now(), None, "没设过的没有");
}

/// 卡住的接着跑（施工 X-8 下）：原因不再给人看，轮数接着数。
#[test]
fn a_blocked_goal_resumes_without_its_reason() {
    let mut events = story();
    events.push(event(19, "goal.resumed", "{}"));
    events.push(started(20));
    let goals = Goals::of(&events);
    let goal = goals.now().expect("接着跑");
    assert_eq!(
        (goal.state, goal.reason.as_deref(), goal.round, goal.resumes),
        (GoalState::Running, None, 4, 2)
    );
}
