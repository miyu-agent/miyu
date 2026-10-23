//! 在跑的工具的进度（施工 X-45 补，`docs/blueprint/view.md` 的 `tool` 条目）：视图流里那一步带上最新的一行进度，结果到了
//! 去掉；翻页没有。同一个调用 0.5 秒里只推一条，压着的到点由 `tick` 推出来；两个调用各算各的。任何工具的调用都认。

use std::sync::Arc;

use miyu_kernel::id::CallId;
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_kernel::time::Timestamp;
use miyu_view::{Body, Change, Projector};

use crate::support::*;

/// 她调 `count` 件，都停在跑着：一件是跑命令，几件是读（读的能一起跑，命令独占）。交回调用编号。
fn running(stage: &mut Stage, count: usize) -> Vec<CallId> {
    let calls: Vec<(&str, &str)> = (0..count)
        .map(|k| match k {
            0 if count == 1 => ("shell", r#"{"command":"./fetch.sh","description":"Fetch"}"#),
            _ => ("read", r#"{"path":"a.txt"}"#),
        })
        .collect();
    stage.model([Line::calls("", &calls), Line::says("ok")]);
    stage.tools((0..count).map(|_| Play::done("done").held()));
    stage.say("fetch");
    stage.held_tools()
}

/// 视图流喂完，交回投影（还要接着 `tick`）和交出的变化。
fn streamed(stage: &Stage) -> (Projector, Vec<Change>) {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut changes = Vec::new();
    for item in stage.stream() {
        changes.extend(match item {
            Streamed::Event(event) => projector.event(event),
            Streamed::Transient(transient) => projector.transient(transient),
        });
    }
    (projector, changes)
}

/// 变化里写过的进度，照先后。
fn shown(changes: &[Change]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|change| match change {
            Change::Update { entry, .. } => match &entry.body {
                Body::Tool(tool) => tool.progress.clone(),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// 那一步现在的进度。
fn progress_of(projector: &Projector) -> Option<String> {
    projector
        .entries()
        .iter()
        .find_map(|entry| match &entry.body {
            Body::Tool(tool) => tool.progress.clone(),
            _ => None,
        })
}

#[test]
fn a_running_step_carries_its_latest_progress_until_the_result() {
    let mut stage = stage();
    let call = running(&mut stage, 1)[0];
    stage.tool_progress(call, "start\nmiyu-progress: 下好 1/3\n", 1000);
    let (projector, changes) = streamed(&stage);
    assert_eq!(progress_of(&projector).as_deref(), Some("下好 1/3"));
    assert_eq!(shown(&changes), ["下好 1/3"]);
    assert!(
        page(&stage).iter().all(|entry| match &entry.body {
            Body::Tool(tool) => tool.progress.is_none(),
            _ => true,
        }),
        "翻页没有"
    );
    stage.release_tool(call);
    let (projector, changes) = streamed(&stage);
    assert_eq!(progress_of(&projector), None, "结果到了去掉");
    assert!(projector.due().is_none());
    let last_tool_update = changes.iter().rev().find_map(|change| match change {
        Change::Update { entry, .. } => match &entry.body {
            Body::Tool(tool) => Some(tool.progress.clone()),
            _ => None,
        },
        _ => None,
    });
    assert_eq!(last_tool_update, Some(None), "最后一次更新不带进度");
}

#[test]
fn within_half_a_second_one_update_and_the_tail_comes_on_time() {
    let mut stage = stage();
    let call = running(&mut stage, 1)[0];
    stage.tool_progress(call, "miyu-progress: 1/4\n", 1000);
    let first = stage.stream().iter().rev().find_map(|item| match item {
        Streamed::Transient(transient) => Some(transient.at),
        Streamed::Event(_) => None,
    });
    stage.tool_progress(call, "miyu-progress: 2/4\n", 100);
    stage.tool_progress(call, "miyu-progress: 3/4\n", 100);
    let (mut projector, changes) = streamed(&stage);
    assert_eq!(shown(&changes), ["1/4"], "0.5 秒里只推一条");
    let first = first.expect("推过一段");
    let due = projector.due().expect("压着一条");
    assert_eq!(due.unix_millis(), first.unix_millis() + 500);
    let early = Timestamp::from_unix_millis(due.unix_millis() - 1).unwrap();
    assert!(projector.tick(early).is_empty(), "没到点不推");
    assert_eq!(shown(&projector.tick(due)), ["3/4"], "到点推最新的那条");
    assert!(projector.due().is_none());
    assert_eq!(progress_of(&projector).as_deref(), Some("3/4"));
}

#[test]
fn the_same_progress_again_is_not_pushed_and_two_calls_count_apart() {
    let mut stage = stage();
    let calls = running(&mut stage, 2);
    stage.tool_progress(calls[0], "miyu-progress: a 1\n", 1000);
    stage.tool_progress(calls[1], "miyu-progress: b 1\n", 10);
    stage.tool_progress(calls[0], "miyu-progress: a 1\n", 1000);
    let (projector, changes) = streamed(&stage);
    assert_eq!(
        shown(&changes),
        ["a 1", "b 1"],
        "两个调用各推各的，一样的不再推"
    );
    assert!(projector.due().is_none());
}

/// 压着一条的时候结果到了：压着的一起扔掉，之后到点也不再写回去。
#[test]
fn a_result_drops_what_was_held_back() {
    let mut stage = stage();
    let call = running(&mut stage, 1)[0];
    stage.tool_progress(call, "miyu-progress: 1/2\n", 1000);
    stage.tool_progress(call, "miyu-progress: 2/2\n", 100);
    stage.release_tool(call);
    let (mut projector, _) = streamed(&stage);
    assert!(projector.due().is_none(), "没有压着的");
    let last = stage.log().last().expect("有事件").at;
    let later = Timestamp::from_unix_millis(last.unix_millis() + 3_600_000).unwrap();
    assert!(projector.tick(later).is_empty(), "到点也不写回去");
    assert_eq!(progress_of(&projector), None);
}
