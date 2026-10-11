//! 长期目标的五种事件（施工 X-6）：图纸上的写法读写一字不差，不写的格子不写；不认识的结局原样留着。

use crate::event::{Body, Event};

/// 一条事件的日志行读进来、再写出去。
fn round_trip(line: &str) -> Event {
    let event = Event::from_line(line).unwrap();
    assert_eq!(event.to_line(), line, "写出去一字不差");
    event
}

#[test]
fn goal_events_read_and_write_as_drawn() {
    let set = round_trip(
        r#"{"seq":4,"at":"2026-10-12T01:00:00.000Z","kind":"goal.set","by":{"kind":"person","account":"alice"},"cause":"cmd-1","body":{"text":"把测试修绿","limit":20}}"#,
    );
    let Body::GoalSet(set) = set.body else {
        panic!("是 goal.set");
    };
    assert_eq!(
        (set.text.as_str(), set.limit.map(|n| n.get())),
        ("把测试修绿", Some(20))
    );
    round_trip(
        r#"{"seq":5,"at":"2026-10-12T01:00:01.000Z","kind":"goal.changed","by":{"kind":"person","account":"alice"},"cause":"cmd-2","body":{"text":"把测试和文档都修好"}}"#,
    );
    round_trip(
        r#"{"seq":6,"at":"2026-10-12T01:00:02.000Z","kind":"goal.paused","by":{"kind":"person","account":"alice"},"cause":"cmd-3","body":{}}"#,
    );
    round_trip(
        r#"{"seq":7,"at":"2026-10-12T01:00:03.000Z","kind":"goal.resumed","by":{"kind":"person","account":"alice"},"cause":"cmd-4","body":{}}"#,
    );
    let ended = round_trip(
        r#"{"seq":8,"at":"2026-10-12T01:00:04.000Z","kind":"goal.ended","turn":7,"by":{"kind":"tool","call_id":"call_7_1"},"body":{"outcome":"blocked","reason":"要人给 key"}}"#,
    );
    assert!(matches!(ended.body, Body::GoalEnded(_)));
}

#[test]
fn an_unknown_outcome_is_kept_as_it_was() {
    let event = round_trip(
        r#"{"seq":8,"at":"2026-10-12T01:00:04.000Z","kind":"goal.ended","by":{"kind":"kernel"},"body":{"outcome":"abandoned"}}"#,
    );
    let Body::GoalEnded(ended) = event.body else {
        panic!("是 goal.ended");
    };
    assert_eq!(ended.outcome.as_str(), "abandoned");
}
