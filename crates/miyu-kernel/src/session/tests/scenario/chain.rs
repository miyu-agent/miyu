//! 被回报叫醒的连着几轮合用一份步数（发布前第三轮审计 R3E-2，`docs/blueprint/kernel/session.md`「回报」第 11 条，2026-11-02
//! 项目主人定）：后台命令、子代理的回报和子代理的留言叫醒的回合，从人上一次说话以后合起来数；用完了，这样的回报只记下、
//! 不开轮；人说一句话重新算。别的不是人开的（别的 harness、别的会话、定时、长期目标、重启接着干）照旧各算各的。
//!
//! 有人看着的会话，不是人开的回合最多请求两次（`steps.rs` 的 [`attended_stage`]），这一串也是两次。

use super::steps::{attended_stage, last_turn, reading};
use super::*;
use crate::event::{ChildReason, EndReason};

/// 三个子代理的会话。
const FIRST: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const SECOND: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92";
const THIRD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d93";

/// 人开的第一轮派出去两个子代理 `j1`、`j2`。
fn two_agents() -> Stage {
    let mut s = attended_stage();
    s.model([
        Line::calls("派出去。", &[("shell", "{}"), ("shell", "{}")]),
        Line::says("派出去了。"),
    ]);
    s.tools([
        Play::starts_agent(1, "查 CI", FIRST),
        Play::starts_agent(2, "查日志", SECOND),
    ]);
    s.say("查一下 CI 和日志");
    s
}

/// 几条 `turn.started`。
fn turns(s: &Stage) -> usize {
    s.log()
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count()
}

/// `j1` 叫醒的一轮请求一次就说完；`j2` 叫醒的一轮又派出 `j3`，这一串到这里用了两步：这一步齐了就结束，`j3` 还没报。
fn spent() -> Stage {
    let mut s = two_agents();
    s.model([Line::says("CI 看到了。")]);
    s.child_reports(1, FIRST, ChildReason::Done, "CI 红在 macOS。");
    assert_eq!(last_turn(&s), (Some(EndReason::Completed), 1));
    // 只给一步：到了上限就不再请求。
    s.model([Line::calls("再派一个。", &[("shell", "{}")])]);
    s.tools([Play::starts_agent(3, "再查", THIRD)]);
    s.child_reports(2, SECOND, ChildReason::Done, "日志里没有。");
    assert_eq!(
        last_turn(&s),
        (Some(EndReason::StepLimit), 1),
        "这一轮自己才一步，和前一轮合起来到了两步：{:#?}",
        story(&s)
    );
    s
}

#[test]
fn woken_turns_share_one_budget_and_a_spent_one_only_records_the_report() {
    let mut s = spent();
    let before = turns(&s);
    s.child_reports(3, THIRD, ChildReason::Done, "也没有。");
    assert_eq!(turns(&s), before, "用完了只记下：{:#?}", story(&s));
    assert!(matches!(
        s.log().last().map(|event| &event.body),
        Some(Body::ChildReported(_))
    ));
}

#[test]
fn a_person_speaking_starts_a_fresh_budget() {
    let mut s = spent();
    s.model([Line::says("还在。")]);
    s.say("还在吗");
    s.model(reading(1));
    s.model([Line::says("都看到了。")]);
    s.tools([Play::done("A")]);
    s.child_reports(3, THIRD, ChildReason::Done, "也没有。");
    assert_eq!(
        last_turn(&s),
        (Some(EndReason::Completed), 2),
        "人说过话，回报叫醒的一轮又有两步：{:#?}",
        story(&s)
    );
}

#[test]
fn other_wakers_still_open_a_turn_once_the_budget_is_spent() {
    let mut s = spent();
    let before = turns(&s);
    s.model([Line::says("收到。")]);
    s.harness_says("ci", "流水线跑完了");
    assert_eq!(turns(&s), before + 1, "{:#?}", story(&s));
    assert_eq!(last_turn(&s), (Some(EndReason::Completed), 1));
}

#[test]
fn the_budget_is_counted_again_from_the_log_after_a_reload() {
    // 用了一步以后重新载入：剩下的一步照日志算回来。
    let mut s = two_agents();
    s.model([Line::says("CI 看到了。")]);
    s.child_reports(1, FIRST, ChildReason::Done, "CI 红在 macOS。");
    s.crash();
    // 只给一步：到了上限就不再请求。
    s.model([Line::calls("再派一个。", &[("shell", "{}")])]);
    s.tools([Play::starts_agent(3, "再查", THIRD)]);
    s.child_reports(2, SECOND, ChildReason::Done, "日志里没有。");
    assert_eq!(
        last_turn(&s),
        (Some(EndReason::StepLimit), 1),
        "{:#?}",
        story(&s)
    );
    // 用完了再载入一次：还是只记下。
    s.crash();
    let before = turns(&s);
    s.child_reports(3, THIRD, ChildReason::Done, "也没有。");
    assert_eq!(turns(&s), before, "{:#?}", story(&s));
    assert!(matches!(
        s.log().last().map(|event| &event.body),
        Some(Body::ChildReported(_))
    ));
}
