//! 包写过的状态（施工 TW-1）：每个包当前的是最近一份没撤掉的；空的是清掉；清空上下文清掉全部，平常的压缩不动；撤销那一轮
//! 退回去，恢复回来；工具结果带的、程序直接写的一样算；压缩以后的有效历史照样记得。

use super::{States, touches};
use crate::event::{
    Body, CompactTrigger, ContextCompacted, Effect, Event, PackageState, State, ToolResult,
    TurnReverted, TurnUnreverted,
};
use crate::history::History;
use crate::id::{Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

fn event(n: u64, turn: u64, body: Body) -> Event {
    Event {
        seq: seq(n),
        at: Timestamp::parse("2026-10-28T07:00:00.000Z").unwrap(),
        turn: Some(TurnId::new(seq(turn))),
        by: By::Kernel,
        cause: None,
        body,
    }
}

/// 包 `package` 的一份，`note` 是 `note`；空的是清掉。
fn written(package: &str, note: &str) -> PackageState {
    PackageState {
        package: package.to_string(),
        state: State {
            note: (!note.is_empty()).then(|| note.to_string()),
            ..State::default()
        },
    }
}

/// 第 `n` 条：第 `turn` 轮的工具结果带上包 `package` 的一份。
fn by_tool(n: u64, turn: u64, package: &str, note: &str) -> Event {
    let result: ToolResult = serde_json::from_value(serde_json::json!({
        "call_id": format!("call_{}_1", n - 1),
        "status": "ok",
        "blocks": [],
    }))
    .unwrap();
    event(
        n,
        turn,
        Body::ToolResult(ToolResult {
            effects: vec![Effect::PackageState(written(package, note))],
            ..result
        }),
    )
}

/// 第 `n` 条：程序直接写的。
fn by_program(n: u64, turn: u64, package: &str, note: &str) -> Event {
    event(n, turn, Body::PackageState(written(package, note)))
}

/// 每个包当前的 `note`，`包=字` 用逗号连起来。
fn current(states: &States) -> String {
    states
        .current()
        .iter()
        .map(|(package, state)| format!("{package}={}", state.note.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join(",")
}

fn reverted(n: u64, turn: u64) -> Event {
    let body: TurnReverted = serde_json::from_value(serde_json::json!({"turns": [turn]})).unwrap();
    event(n, turn, Body::TurnReverted(body))
}

fn unreverted(n: u64, turn: u64) -> Event {
    let body: TurnUnreverted =
        serde_json::from_value(serde_json::json!({"turns": [turn]})).unwrap();
    event(n, turn, Body::TurnUnreverted(body))
}

#[test]
fn each_package_keeps_its_latest_state_not_undone() {
    let mut states = States::default();
    assert_eq!(current(&states), "", "没写过的没有");
    states.note(&by_tool(5, 3, "todo", "a"));
    states.note(&by_program(6, 3, "plan", "p"));
    states.note(&by_tool(9, 7, "todo", "b"));
    assert_eq!(current(&states), "plan=p,todo=b");
    states.note(&reverted(10, 7));
    assert_eq!(current(&states), "plan=p,todo=a", "撤掉第 7 轮写的");
    states.note(&unreverted(11, 7));
    assert_eq!(current(&states), "plan=p,todo=b", "恢复回来");
    states.note(&by_program(12, 11, "todo", ""));
    assert_eq!(current(&states), "plan=p", "空的是清掉");
}

/// 压缩的检查点：`trigger` 是 `trigger`，替代到第 `upto` 条。
fn compacted(n: u64, turn: u64, upto: u64, trigger: CompactTrigger) -> Event {
    event(
        n,
        turn,
        Body::ContextCompacted(ContextCompacted {
            upto: seq(upto),
            summary: String::new(),
            trigger: Some(trigger),
            instructions: None,
            notes: String::new(),
            restored: Vec::new(),
            refills: None,
            prepared: false,
        }),
    )
}

#[test]
fn only_clearing_the_context_clears_every_package() {
    let mut states = States::default();
    states.note(&by_tool(5, 3, "todo", "a"));
    states.note(&by_program(6, 3, "plan", "p"));
    states.note(&compacted(8, 7, 6, CompactTrigger::Auto));
    assert_eq!(current(&states), "plan=p,todo=a", "平常的压缩照旧");
    states.note(&compacted(10, 9, 8, CompactTrigger::Clear));
    assert_eq!(current(&states), "", "清空上下文的清掉全部");
    states.note(&by_tool(11, 10, "todo", "c"));
    assert_eq!(current(&states), "todo=c", "清了以后写的照记");
    states.note(&reverted(12, 9));
    assert_eq!(current(&states), "plan=p,todo=c", "撤掉清空的那一轮");
}

#[test]
fn history_after_a_checkpoint_still_knows_the_states() {
    let mut history = History::default();
    history.append(by_tool(5, 3, "todo", "a"));
    let after = history.after(seq(5));
    assert_eq!(
        after
            .package_states()
            .get("todo")
            .and_then(|state| state.note.as_deref()),
        Some("a"),
        "写它的那一条被替代掉了，状态照样在"
    );
}

#[test]
fn only_writes_clears_and_undo_touch_the_states() {
    assert!(touches(&by_tool(5, 3, "todo", "a")));
    assert!(touches(&by_program(5, 3, "todo", "a")));
    assert!(touches(&reverted(5, 3)));
    assert!(touches(&unreverted(5, 3)));
    assert!(touches(&compacted(5, 3, 4, CompactTrigger::Clear)));
    assert!(!touches(&compacted(5, 3, 4, CompactTrigger::Auto)));
}
