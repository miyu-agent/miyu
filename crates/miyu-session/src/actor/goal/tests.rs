//! 续轮的账（施工 X-7）：续轮开的一轮没调工具就等人，调了的不等；人说一句不等了；设、接着跑以后重新发完整版；别处开的轮不算
//! 续轮。

use super::*;

/// 一条事件：序号、种类、`by`、回合编号、`body`。
fn event(seq: u64, kind: &str, by: &str, turn: Option<u64>, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    serde_json::from_str(&format!(
        r#"{{"seq":{seq},"at":"2026-10-12T01:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
    ))
    .expect("合写法")
}

const GOAL: &str = r#"{"kind":"module","id":"goal"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;

/// 一轮：第 `seq` 条是触发它的话（`by`），接着开一轮，回复里调不调工具，结束。
fn round(seq: u64, by: &str, calls: bool) -> Vec<Event> {
    let turn = seq + 1;
    let blocks = if calls {
        format!(
            r#"[{{"type":"tool_call","call_id":"call_{}_1","name":"read","args":"{{}}"}}]"#,
            seq + 2
        )
    } else {
        r#"[{"type":"text","text":"好"}]"#.to_string()
    };
    let mut events = vec![
        event(
            seq,
            "message.user",
            by,
            None,
            r#"{"blocks":[{"type":"text","text":"x"}]}"#,
        ),
        event(
            turn,
            "turn.started",
            KERNEL,
            Some(turn),
            &format!(r#"{{"trigger":{seq}}}"#),
        ),
        event(
            seq + 2,
            "message.assistant",
            "{\"kind\":\"model\",\"endpoint\":\"p\",\"model\":\"m\"}",
            Some(turn),
            &format!(r#"{{"blocks":{blocks},"seen":{}}}"#, seq + 1),
        ),
    ];
    events.push(event(
        seq + 3,
        "turn.ended",
        KERNEL,
        Some(turn),
        r#"{"reason":"completed"}"#,
    ));
    events
}

#[test]
fn a_continuation_without_tools_waits_until_a_person_speaks() {
    let mut run = GoalRun::default();
    run.note(&[event(2, "goal.set", ALICE, None, r#"{"text":"修绿"}"#)]);
    run.note(&round(3, GOAL, true));
    assert!(!run.waiting, "调了工具的不等");
    run.note(&round(7, ALICE, false));
    assert!(!run.waiting, "人开的那一轮没调工具不算");
    run.note(&round(11, GOAL, false));
    assert!(run.waiting, "续轮没调工具，等人");
    run.note(&round(15, ALICE, false));
    assert!(!run.waiting, "人说了一句，不等了");
}

#[test]
fn setting_or_resuming_starts_with_the_full_message_again() {
    let mut run = GoalRun {
        full_sent: true,
        waiting: true,
        sent_at: Some(3),
        ..GoalRun::default()
    };
    run.note(&[event(2, "goal.resumed", ALICE, None, "{}")]);
    assert!(!run.full_sent && !run.waiting && run.sent_at.is_none());
    run.full_sent = true;
    run.note(&[event(3, "goal.changed", ALICE, None, r#"{"text":"别的"}"#)]);
    assert!(!run.full_sent, "改了的也重新发完整版");
}
