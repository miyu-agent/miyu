//! 工具要结束这一轮（施工 4-14，`kernel/session.md`「工具怎么调、下一步怎么走」）：这一步几件的结果都到了，有一件要结束就不再
//! 请求模型，照走完了记 `turn.ended`（`by` 是要结束的那一次调用）；同一步别的照常跑完；出错的不算；这一轮里排着的照常接着
//! 开下一轮；有计划的重启碰上要结束的这一步，照走完了收，再起来不接着干。

use super::executor::*;
use super::load::{Logged, ended_with, load};
use super::*;
use crate::event::{EndReason, ToolStatus};
use crate::id::CallId;
use crate::origin::Tool;

/// 调用 `call_id` 成功，交回 `text`，要结束这一轮。
fn ending(call_id: CallId, text: &str) -> Input {
    let mut input = done(call_id, text);
    if let Input::ToolDone { end_turn, .. } = &mut input {
        *end_turn = true;
    }
    input
}

/// 这一条 `tool.result` 要不要结束这一轮。
fn ends(event: &Event) -> bool {
    matches!(&event.body, Body::ToolResult(result) if result.end_turn)
}

#[test]
fn a_call_that_ends_the_turn_stops_further_requests() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}")]);
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, ending(call(6, 1), "skipped"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9]));
    assert!(ends(&events[0]), "结果上记着要结束");
    assert_eq!(reason_of(&events[1]), &EndReason::Completed);
    assert_eq!(
        events[1].by,
        By::Tool(Tool {
            call_id: call(6, 1)
        })
    );
    let actions = allowing(&mut session, stored(9));
    assert!(calls(&actions).is_empty(), "不再请求");
}

#[test]
fn the_other_calls_of_the_step_finish_first() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}"), ("read", "{}")]);
    allowing(&mut session, stored(7));
    let actions = allowing(&mut session, ending(call(6, 1), "skipped"));
    assert_eq!(appended(&actions), seqs(&[8]), "另一件还在跑：先不结束");
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::CancelTool { .. }))
    );
    let actions = allowing(&mut session, done(call(6, 2), "remembered"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    assert!(!ends(&events[0]));
    assert_eq!(reason_of(&events[1]), &EndReason::Completed);
    assert_eq!(
        events[1].by,
        By::Tool(Tool {
            call_id: call(6, 1)
        }),
        "要结束的是先到的那一件"
    );
}

#[test]
fn a_failed_call_does_not_end_the_turn() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}")]);
    allowing(&mut session, stored(7));
    let mut failed = ending(call(6, 1), "boom");
    if let Input::ToolDone { error, .. } = &mut failed {
        *error = true;
    }
    let actions = allowing(&mut session, failed);
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]));
    assert!(
        matches!(&events[0].body, Body::ToolResult(result) if result.status == ToolStatus::Error)
    );
    assert!(!ends(&events[0]), "出错的不记");
    let actions = allowing(&mut session, stored(8));
    assert_eq!(calls(&actions).first().map(|(seen, _)| *seen), Some(seq(8)));
}

#[test]
fn a_message_queued_meanwhile_opens_the_next_turn() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}")]);
    allowing(&mut session, stored(7));
    allowing(&mut session, send(2, "又 @ 她了"));
    allowing(&mut session, stored(8));
    let actions = allowing(&mut session, ending(call(6, 1), "skipped"));
    let events = appended_events(&actions);
    assert_eq!(reason_of(&events[1]), &EndReason::Completed);
    assert!(
        matches!(&events[2].body, Body::TurnStarted(started) if started.trigger == Some(seq(8))),
        "排着的那一句接着开一轮：{events:?}"
    );
}

#[test]
fn a_planned_restart_after_an_ending_call_does_not_resume() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}"), ("read", "{}")]);
    logged.handle(ending(call(6, 1), "skipped"));
    logged.handle(stored(logged.last()));
    let actions = logged.handle(Input::Restarting { at: at(53) });
    let events = appended_events(&actions);
    let ended = events.last().expect("结束了");
    assert_eq!(ended_with(ended), &EndReason::Completed, "照走完了收");
    logged.handle(stored(logged.last()));
    let (_, actions) = load(logged.log.clone());
    assert!(
        !appended_events(&actions)
            .iter()
            .any(|event| matches!(event.body, Body::TurnStarted(_))),
        "再起来不接着干"
    );
}

#[test]
fn the_flag_is_written_only_when_set() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}"), ("read", "{}")]);
    allowing(&mut session, stored(7));
    let ended = appended_events(&allowing(&mut session, ending(call(6, 1), "a")));
    let plain = appended_events(&allowing(&mut session, done(call(6, 2), "b")));
    let json = |event: &Event| serde_json::to_value(&event.body).expect("写得出");
    assert_eq!(json(&ended[0])["end_turn"], true);
    assert!(json(&plain[0]).get("end_turn").is_none(), "不要的不写");
    let old: crate::event::ToolResult =
        serde_json::from_str(r#"{"call_id":"call_6_2","status":"ok","blocks":[]}"#)
            .expect("老日志读得出");
    assert!(!old.end_turn);
}
