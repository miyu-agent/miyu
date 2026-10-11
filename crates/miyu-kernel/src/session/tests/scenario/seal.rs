//! 跑完几轮封存（施工 C-8 下，`docs/blueprint/session/fork.md`「封存」）：跑满 `seal_after` 轮、这一轮结束、没有接着开的，
//! 同一批记 `session.sealed`；跑的时候发来的、被有计划的重启打断以后接着干的不另算一轮；崩了载入时收尾的那一轮、重启太多次
//! 不再接着干的那一轮也封；载入接着数。封存以后改对话的命令拒 `session_sealed`，改标题照常；后台命令结束只记下、不开轮。

use std::num::NonZeroU32;

use super::*;
use crate::event::JobReason;
use crate::origin::Tool;

/// 父会话说完一轮，从它最后一条分叉，跑满 `after` 轮封存（没有的不封）。
fn branch(after: Option<u32>) -> Stage {
    let mut parent = stage();
    parent.model([Line::says("好。")]);
    parent.say("你好");
    let after = after.and_then(NonZeroU32::new);
    Stage::forked(
        policy,
        environment("~/src/miyu"),
        at(30),
        parent.log(),
        after,
    )
}

/// 日志里有几条 `session.sealed`。
fn seals(s: &Stage) -> usize {
    s.log()
        .iter()
        .filter(|event| matches!(event.body, Body::SessionSealed(_)))
        .count()
}

/// 最后两条：`turn.ended`、`session.sealed`，封存的那一条是内核记的、`cause` 同那一条结束、不带回合编号。
fn sealed_right_after_the_round(s: &Stage) {
    let log = s.log();
    let (ended, sealed) = (&log[log.len() - 2], &log[log.len() - 1]);
    assert!(matches!(ended.body, Body::TurnEnded(_)), "{:#?}", story(s));
    assert!(
        matches!(sealed.body, Body::SessionSealed(_)),
        "{:#?}",
        story(s)
    );
    assert_eq!((&sealed.by, &sealed.cause), (&By::Kernel, &ended.cause));
    assert_eq!(sealed.turn, None);
    assert_eq!(seals(s), 1, "只封一次");
}

/// 命令 `id` 拒了 `session_sealed`。
fn refused(s: &Stage, id: &CommandId) {
    assert_eq!(
        s.outcome(id),
        Some(&Outcome::Rejected {
            reason: Reason::Sealed,
            about: Vec::new()
        })
    );
}

#[test]
fn a_branch_is_sealed_when_its_round_ends() {
    let mut s = branch(Some(1));
    s.model([Line::says("支线。")]);
    s.say("再来");
    sealed_right_after_the_round(&s);
    let before = s.log().len();
    let id = s.say("还在吗");
    refused(&s, &id);
    assert_eq!(s.log().len(), before, "拒了什么都不记");
    let id = s.set_meta(Some("支线"), None);
    assert!(
        matches!(s.outcome(&id), Some(Outcome::Accepted { .. })),
        "改标题照常"
    );
}

#[test]
fn a_branch_sealed_after_two_rounds_waits_for_the_second() {
    let mut s = branch(Some(2));
    s.model([Line::says("一。"), Line::says("二。")]);
    s.say("一");
    assert_eq!(seals(&s), 0, "还差一轮");
    s.say("二");
    sealed_right_after_the_round(&s);
}

#[test]
fn a_branch_without_seal_after_runs_on() {
    let mut s = branch(None);
    s.model([Line::says("一。"), Line::says("二。")]);
    s.say("一");
    s.say("二");
    assert_eq!(seals(&s), 0);
    assert_eq!(s.turns().len(), 2);
}

#[test]
fn clearing_the_context_is_not_a_round() {
    let mut s = branch(Some(3));
    s.model([Line::says("一。"), Line::says("二。"), Line::says("三。")]);
    s.say("一");
    s.request_clear();
    s.say("二");
    assert_eq!(seals(&s), 0, "清空单开的那一轮没有触发，不算");
    s.say("三");
    sealed_right_after_the_round(&s);
}

#[test]
fn a_message_sent_during_the_round_runs_on_in_it() {
    let mut s = branch(Some(2));
    s.model([
        Line::says("一。").held(),
        Line::says("二。"),
        Line::says("三。"),
    ]);
    s.say("再来");
    s.say("还有");
    s.release_model();
    assert_eq!(s.turns().len(), 2, "排着队的那条接着开了一轮");
    assert_eq!(seals(&s), 0, "接着开的不另算，还差一轮");
    s.say("再一轮");
    sealed_right_after_the_round(&s);
}

#[test]
fn a_round_resumed_after_a_planned_restart_is_the_same_round() {
    let mut s = branch(Some(2));
    s.model([
        Line::says("一。").held(),
        Line::says("接着。"),
        Line::says("二。"),
    ]);
    s.say("再来");
    s.restart();
    assert_eq!(s.turns().len(), 2, "重启以后接着干");
    assert_eq!(seals(&s), 0, "接着干的不另算，还差一轮");
    s.say("再一轮");
    sealed_right_after_the_round(&s);
}

#[test]
fn a_round_no_longer_resumed_after_too_many_restarts_is_sealed() {
    let mut s = branch(Some(1));
    s.model(["一。", "二。", "三。", "四。"].map(|said| Line::says(said).held()));
    s.say("再来");
    // 测试的策略接着干三次（`resumes`）：第四次重启以后不再接着干，这一轮算跑完了。
    for _ in 0..4 {
        s.restart();
    }
    sealed_right_after_the_round(&s);
}

#[test]
fn a_round_cut_by_a_crash_is_sealed_on_load() {
    let mut s = branch(Some(1));
    s.model([Line::says("一。").held()]);
    s.say("再来");
    s.crash();
    sealed_right_after_the_round(&s);
    let id = s.say("还在吗");
    refused(&s, &id);
}

#[test]
fn a_loaded_branch_counts_on_from_the_log() {
    let mut s = branch(Some(2));
    s.model([Line::says("一。")]);
    s.say("一");
    s.crash();
    assert_eq!(seals(&s), 0);
    s.model([Line::says("二。")]);
    s.say("二");
    sealed_right_after_the_round(&s);
    s.crash();
    let id = s.say("三");
    refused(&s, &id);
}

#[test]
fn a_command_ending_after_the_seal_is_only_recorded() {
    let mut s = branch(Some(1));
    s.model([
        Line::calls("放出去。", &[("shell", "{}")]),
        Line::says("放出去了。"),
    ]);
    s.tools([Play::starts_command(1, "跑测试")]);
    s.say("跑测试");
    sealed_right_after_the_round(&s);
    let call_id = s.ran()[0].0;
    s.job_ends(1, JobReason::Exited, By::Tool(Tool { call_id }), None);
    let last = s.log().last().expect("有事件");
    assert!(
        matches!(last.body, Body::JobReported(_)),
        "{:#?}",
        story(&s)
    );
    assert_eq!(s.turns().len(), 1, "只记下，不开轮");
}
