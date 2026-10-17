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

/// 回报叫醒的一轮（施工 X-7 再补）：第 `seq` 条是后台命令的回报，接着由它开一轮，回复里调不调工具，结束。
fn woken(seq: u64, calls: bool) -> Vec<Event> {
    let mut events = round(seq, GOAL, calls);
    events[0] = event(
        seq,
        "job.reported",
        r#"{"kind":"tool","call_id":"call_3_1"}"#,
        None,
        r#"{"job":"j1","reason":"exited","exit_code":0,"duration_ms":10,"output":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","chars":0}"#,
    );
    events
}

#[test]
fn a_turn_woken_by_a_report_that_calls_tools_stops_waiting() {
    let mut run = GoalRun {
        waiting: true,
        ..GoalRun::default()
    };
    run.note(&woken(20, false));
    assert!(run.waiting, "叫醒的一轮没调工具，照旧等人");
    run.note(&woken(30, true));
    assert!(!run.waiting, "叫醒的一轮调了工具，不再等人");
    run.note(&woken(40, false));
    assert!(!run.waiting, "叫醒的一轮没调工具不算等人");
}

#[test]
fn running_commands_hold_the_round_until_the_fallback() {
    let mut run = GoalRun::default();
    let start = Instant::now();
    let until = start + FALLBACK;
    assert_eq!(run.hold(start), Some(until), "头一回看到，等满保底");
    assert_eq!(run.hold(start + FALLBACK / 2), Some(until), "起点不跟着挪");
    assert_eq!(run.hold(until), None, "等满了照常开");
    run.note(&round(50, GOAL, true));
    let later = until + FALLBACK / 4;
    assert_eq!(run.hold(later), Some(later + FALLBACK), "开了一轮重新算");
}
