//! 步数上限照开轮的是谁（施工 AE-1，`docs/blueprint/kernel/session.md`「工具怎么调、下一步怎么走」第 7 条），真会话一整轮一整轮
//! 地跑：有人看着的会话里，重启接着干的一轮、回报叫醒的一轮都不是人开的，照「自己开的回合」那个数停（2026-10-30 主会话定：
//! 只有人开的不限）。

use super::reports::{CHILD, dispatched};
use super::*;
use crate::event::{ChildReason, EndReason};

/// 有人看着：人开的回合不限，别的最多请求两次。
fn attended() -> Policy {
    let mut policy = policy();
    policy.step_limit = None;
    policy.auto_step_limit = Some(2);
    policy
}

fn attended_stage() -> Stage {
    Stage::new(attended, environment("~/src/miyu"), at(0))
}

/// 一直读：`n` 次调 `read`。
fn reading(n: usize) -> Vec<Line> {
    (0..n)
        .map(|_| Line::calls("再读。", &[("read", r#"{"path":"a"}"#)]))
        .collect()
}

/// 最后一轮：怎么结束的、请求了几次。
fn last_turn(stage: &Stage) -> (Option<EndReason>, usize) {
    let log = stage.log();
    let started = log
        .iter()
        .rposition(|event| matches!(event.body, Body::TurnStarted(_)))
        .expect("开过一轮");
    let called = log[started..]
        .iter()
        .filter(|event| matches!(event.body, Body::ModelCalled(_)))
        .count();
    let ended = log[started..].iter().find_map(|event| match &event.body {
        Body::TurnEnded(ended) => Some(ended.reason.clone()),
        _ => None,
    });
    (ended, called)
}

#[test]
fn a_turn_picked_up_after_a_restart_stops_at_the_limit() {
    let mut stage = attended_stage();
    stage.model([Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)])]);
    stage.model(reading(3));
    stage.tools([Play::done("A").held()]);
    stage.tools((0..3).map(|_| Play::done("A")));
    stage.say("读 a");
    stage.restart();
    assert_eq!(
        last_turn(&stage),
        (Some(EndReason::StepLimit), 2),
        "{:#?}",
        story(&stage)
    );
}

#[test]
fn a_turn_a_report_wakes_stops_at_the_limit() {
    let mut s = dispatched(attended_stage());
    s.model(reading(3));
    s.tools((0..3).map(|_| Play::done("A")));
    s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    assert_eq!(
        last_turn(&s),
        (Some(EndReason::StepLimit), 2),
        "{:#?}",
        story(&s)
    );
}

#[test]
fn a_turn_a_person_opens_is_not_limited() {
    let mut stage = attended_stage();
    stage.model(reading(4));
    stage.model([Line::says("读完了。")]);
    stage.tools((0..4).map(|_| Play::done("A")));
    stage.say("一直读");
    assert_eq!(last_turn(&stage), (Some(EndReason::Completed), 5));
}
