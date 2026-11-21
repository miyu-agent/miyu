//! 晚订阅的人看到的 `doing`（施工 LM-25）：运行中刷新网页、从后台任务点进子代理的头只拿到补发的落了盘的事件，工具在跑时
//! 照样是 `tool`、对着同一步，和一直订阅着的人此刻看到的一样；工具有了结果不再写它，下一块开始前两边都不写。

use std::sync::Arc;

use miyu_kernel::event::{Body as EventBody, Event};
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_view::{Doing, EntryId, Projector, State, Status};

use crate::support::*;

/// 一直订阅着的：落了盘的和瞬时的照推送的先后喂，喂到 `until` 那一条（含）为止，没有的喂完。
fn live_until(stage: &Stage, until: Option<&Event>) -> Status {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for item in stage.stream() {
        match item {
            Streamed::Event(event) => {
                drop(projector.event(event));
                if until.is_some_and(|until| until.seq == event.seq) {
                    break;
                }
            }
            Streamed::Transient(transient) => drop(projector.transient(transient)),
        }
    }
    projector.status()
}

/// 晚订阅的：只喂落了盘的，喂到 `until` 那一条（含）为止。
fn late_until(stage: &Stage, until: Option<&Event>) -> Status {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        drop(projector.event(event));
        if until.is_some_and(|until| until.seq == event.seq) {
            break;
        }
    }
    projector.status()
}

/// 参数里有 `needle` 的那一步工具。
fn step(stage: &Stage, needle: &str) -> EntryId {
    let entries = page(stage);
    of_kind(&entries, "tool")
        .into_iter()
        .find(|entry| {
            json(entry)["args"]
                .as_str()
                .is_some_and(|a| a.contains(needle))
        })
        .map(|entry| entry.id.clone())
        .unwrap_or_else(|| panic!("没有参数里有 {needle} 的一步"))
}

#[test]
fn a_late_subscriber_sees_the_tool_that_is_running() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[("shell", r#"{"command":"sleep 45","description":"Slow"}"#)],
        ),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("42").held()]);
    stage.say("run it");
    let running = Some(Doing::Tool {
        entry: step(&stage, "sleep 45"),
    });
    let live = live_until(&stage, None);
    assert_eq!(live.doing, running, "一直订阅着的：{live:?}");
    let late = late_until(&stage, None);
    assert_eq!(late.state, State::Running);
    assert_eq!(late.doing, running, "晚订阅的：{late:?}");
}

#[test]
fn when_one_of_two_tools_finishes_both_point_at_the_one_still_running() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[
                ("read", r#"{"file_path":"slow"}"#),
                ("read", r#"{"file_path":"fast"}"#),
            ],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("A").held(), Play::done("B")]);
    stage.say("read both");
    let running = Some(Doing::Tool {
        entry: step(&stage, "slow"),
    });
    assert_eq!(live_until(&stage, None).doing, running);
    assert_eq!(late_until(&stage, None).doing, running);
}

#[test]
fn a_finished_tool_is_no_longer_what_is_being_done() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("read");
    let result = stage
        .log()
        .iter()
        .find(|event| matches!(event.body, EventBody::ToolResult(_)))
        .expect("有工具结果")
        .clone();
    for status in [
        live_until(&stage, Some(&result)),
        late_until(&stage, Some(&result)),
    ] {
        assert_eq!(status.state, State::Running, "{status:?}");
        assert_eq!(status.doing, None, "结果到了、下一块还没开始：{status:?}");
    }
    assert_eq!(late_until(&stage, None).doing, None, "一轮结束不写");
}
