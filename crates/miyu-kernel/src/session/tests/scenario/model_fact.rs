//! 场景：模型那一块事实（施工 E-M，`docs/blueprint/kernel/request.md`「事实」）。第一轮跟着环境、权限注入；同一个身份的轮
//! 不再注入；轮和轮之间换了的，下一轮开头追加；回合中途换了的，这一轮下一次请求之前追加；没交身份的不注入。

use super::*;
use crate::facts::Identity;
use crate::session::Limits;

/// 替身的策略加上模型那一份模板：短，一眼认得出是哪个字段。
fn with_model() -> Policy {
    let mut policy = policy();
    policy.facts = policy
        .facts
        .with_model(Some(r#"<m n="{name}" e="{effort}"/>"#))
        .unwrap();
    policy
}

fn stage() -> Stage {
    Stage::new(with_model, environment("~/src/miyu"), at(0))
}

/// 交一份带身份的限额：模型名 `name`、强度 `effort`。
fn hand(stage: &mut Stage, name: &str, effort: &str) {
    stage.limits_with(Limits {
        model: crate::testkit::model(),
        window: None,
        max_output: None,
        images: None,
        blind: false,
        identity: Some(Identity {
            provider: "P".to_string(),
            model: name.to_string(),
            effort: effort.to_string(),
            preset: "none".to_string(),
        }),
    });
}

/// 日志里内核注入的模型那几块：原文和它在第几个开过的回合（从 0 数），照先后。
fn told(stage: &Stage) -> Vec<(String, usize)> {
    let turns = stage.turns();
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "model" => {
                let turn = event.turn.expect("事实带着回合");
                let index = turns.iter().position(|opened| *opened == turn);
                Some((fact.text.clone(), index.expect("是开过的回合")))
            }
            _ => None,
        })
        .collect()
}

fn block(name: &str, effort: &str, turn: usize) -> (String, usize) {
    (format!(r#"<m n="{name}" e="{effort}"/>"#), turn)
}

#[test]
fn it_is_told_once_and_again_after_a_change_between_turns() {
    let mut stage = stage();
    hand(&mut stage, "a", "default");
    stage.model([Line::says("好。"), Line::says("嗯。"), Line::says("在。")]);
    stage.say("hi");
    stage.say("再说一句");
    assert_eq!(told(&stage), [block("a", "default", 0)]);
    hand(&mut stage, "a", "high");
    stage.say("现在呢");
    assert_eq!(
        told(&stage),
        [block("a", "default", 0), block("a", "high", 2)]
    );
}

#[test]
fn a_change_mid_turn_is_told_before_the_next_request_of_that_turn() {
    let mut stage = stage();
    hand(&mut stage, "a", "default");
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    let reading = stage.ran()[0].0;
    hand(&mut stage, "b", "default");
    stage.release_tool(reading);
    assert_eq!(
        told(&stage),
        [block("a", "default", 0), block("b", "default", 0)]
    );
    let log = stage.log();
    let last = log
        .iter()
        .rev()
        .find(|event| matches!(&event.body, Body::ContextInjected(fact) if fact.kind.as_str() == "model"))
        .expect("有换了以后那一块");
    assert!(stage.requests()[1].0 >= last.seq, "{:?}", story(&stage));
}

#[test]
fn handing_the_same_identity_again_mid_turn_tells_nothing() {
    let mut stage = stage();
    hand(&mut stage, "a", "default");
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    let reading = stage.ran()[0].0;
    hand(&mut stage, "a", "default");
    stage.release_tool(reading);
    assert_eq!(told(&stage), [block("a", "default", 0)]);
}

#[test]
fn without_an_identity_nothing_is_told() {
    let mut stage = stage();
    stage.model([Line::says("好。")]);
    stage.say("hi");
    assert!(told(&stage).is_empty(), "{:?}", story(&stage));
}
