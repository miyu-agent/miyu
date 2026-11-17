//! 回合开始告诉她这一轮的话从哪来（施工 VN-1，`facts/channel.rs`）：开这一轮的那句话是哪种头上说的、到点叫醒的，事实里多一块，
//! 排在环境、权限后面；没有这几句的策略（场所会话、以前造的快照）不多。

use std::collections::BTreeMap;

use super::*;
use crate::facts::{CHANNEL, Channels};

/// 替身的策略带上这几句。
fn with_channels() -> Policy {
    let mut policy = policy();
    policy.facts = policy.facts.with_channels(Some(Channels {
        heads: BTreeMap::from([("web".to_string(), "<c>web</c>".to_string())]),
        other: "<c>other</c>".to_string(),
        nobody: "<c>nobody</c>".to_string(),
    }));
    policy
}

/// 编号是 `n` 的命令：`by` 说一句「hi」。
fn said_by(n: u64, by: &str) -> Input {
    let Input::Command(mut received) = send(n, "hi") else {
        unreachable!("send 交的是命令");
    };
    received.by = serde_json::from_str(by).unwrap();
    Input::Command(received)
}

/// 这一批里内核记的这一类的几块。
fn channel_facts(actions: &[Action]) -> Vec<String> {
    appended_events(actions)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == CHANNEL => {
                assert_eq!(event.by, By::Kernel);
                Some(fact.text.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_turn_opened_on_the_web_says_so() {
    let mut session = session_with(with_channels());
    let actions = session.handle(said_by(1, r#"{"kind":"person","head":"web"}"#));
    assert_eq!(channel_facts(&actions), ["<c>web</c>"]);
    let kinds: Vec<String> = appended_events(&actions)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) => Some(fact.kind.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["env", "permission", CHANNEL], "排在环境、权限后面");
}

#[test]
fn a_turn_opened_by_a_schedule_says_nobody_is_there() {
    let mut session = session_with(with_channels());
    let looped = r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#;
    let actions = session.handle(said_by(1, looped));
    assert_eq!(channel_facts(&actions), ["<c>nobody</c>"]);
}

#[test]
fn without_the_lines_nothing_more_is_said() {
    let mut session = session();
    let actions = session.handle(said_by(1, r#"{"kind":"person","head":"web"}"#));
    assert!(channel_facts(&actions).is_empty());
}
