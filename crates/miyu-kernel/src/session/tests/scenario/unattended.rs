//! 场景：到点的定时任务交进没人看着的一次性会话（施工 SC-1，`internal/blueprint/kernel/session.md`「定时交进来的话」）。人建的
//! 定时任务到点一律开一轮；她自己在那种会话里建的、循环照旧只记下。回应里附不附 `turn.started`，核心照它记这一次开没开轮。

use super::reports::last_request;
use super::*;
use crate::origin::{Schedule, ScheduleLoop};

/// 没人看着的一次性会话：`miyu ask` 开的，这时没有头订阅着。
fn unwatched() -> Stage {
    Stage::oneshot(policy, environment("~/src/miyu"), at(0))
}

/// 人建的定时任务到点交进来的那一句的 `by`。
fn hers_by_a_person() -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: None,
        person: true,
    })
}

/// 她自己建的定时任务。
fn her_own() -> By {
    By::Schedule(Schedule {
        schedule: "t2".to_string(),
        looping: None,
        person: false,
    })
}

/// 这个命令的回应里附了几条事件。
fn replied(s: &Stage, id: &CommandId) -> usize {
    match s.outcome(id) {
        Some(Outcome::Accepted { events }) => events.len(),
        other => panic!("没接受：{other:?}"),
    }
}

#[test]
fn a_task_a_person_made_opens_a_turn_though_nobody_watches() {
    let mut s = unwatched();
    s.model([Line::says("看过了，没事。")]);
    let id = s.says_as(hers_by_a_person(), "看一眼磁盘。");
    assert_eq!(s.turns().len(), 1, "人建的到点一律开一轮：{:#?}", story(&s));
    assert_eq!(replied(&s, &id), 2, "回应附上那一句和由它开的那一轮");
}

#[test]
fn her_own_task_there_is_only_recorded() {
    let mut s = unwatched();
    let id = s.says_as(her_own(), "看一眼磁盘。");
    assert!(s.turns().is_empty(), "她自己建的照旧只记下");
    assert_eq!(replied(&s, &id), 1, "没开轮：回应只有那一句");

    s.watched(true);
    s.model([Line::says("看到了。")]);
    s.say("在吗");
    assert_eq!(s.turns().len(), 1);
    assert!(
        last_request(&s).contains("2 message.user\n"),
        "只记下的等人开口一起看到"
    );
}

#[test]
fn a_loop_a_person_started_there_is_only_recorded() {
    let mut s = unwatched();
    let by = By::Schedule(Schedule {
        schedule: "t3".to_string(),
        looping: Some(ScheduleLoop::Fixed),
        person: true,
    });
    s.says_as(by, "再看一眼。");
    assert!(s.turns().is_empty(), "循环不算：没人看着的不由它一轮一轮跑");
}

#[test]
fn one_that_arrives_mid_turn_opens_the_next_turn() {
    let mut s = unwatched();
    s.model([Line::says("我先……").held(), Line::says("这件也看了。")]);
    s.says_as(hers_by_a_person(), "看一眼磁盘。");
    let id = s.says_as(hers_by_a_person(), "再看一眼内存。");
    assert_eq!(replied(&s, &id), 1, "正忙：排着，这时还没由它开轮");
    s.release_model();
    assert_eq!(s.turns().len(), 2, "这一轮结束由它接着开：{:#?}", story(&s));
}

#[test]
fn her_own_that_arrives_mid_turn_does_not_open_the_next() {
    let mut s = unwatched();
    s.model([Line::says("我先……").held()]);
    s.says_as(hers_by_a_person(), "看一眼磁盘。");
    s.says_as(her_own(), "再看一眼内存。");
    s.release_model();
    assert_eq!(s.turns().len(), 1, "她自己建的不接着开：{:#?}", story(&s));
}

#[test]
fn set_aside_while_an_undo_can_be_restored_it_opens_once_restored_even_across_a_reload() {
    let mut s = unwatched();
    s.model([Line::says("好。"), Line::says("嗯。")]);
    s.say("hi");
    s.say("再来");
    let second = s.turns()[1];
    s.revert(second);
    s.says_as(her_own(), "看一眼内存。");
    s.says_as(hers_by_a_person(), "看一眼磁盘。");
    assert_eq!(s.turns().len(), 2, "还能恢复撤销：只记下");
    s.crash();
    s.model([Line::says("看过了。")]);
    s.unrevert();
    assert_eq!(
        s.turns().len(),
        3,
        "记在一边的照日志里的 by 算回来，恢复了由人建的那条接着开：{:#?}",
        story(&s)
    );
}

#[test]
fn her_own_set_aside_does_not_open_once_restored() {
    let mut s = unwatched();
    s.model([Line::says("好。"), Line::says("嗯。")]);
    s.say("hi");
    s.say("再来");
    let second = s.turns()[1];
    s.revert(second);
    s.says_as(her_own(), "看一眼内存。");
    s.unrevert();
    assert_eq!(
        s.turns().len(),
        2,
        "她自己建的恢复了也不开：{:#?}",
        story(&s)
    );
}

#[test]
fn a_watched_session_opens_for_either() {
    let mut s = stage();
    s.model([Line::says("好。"), Line::says("嗯。")]);
    let first = s.says_as(her_own(), "看一眼磁盘。");
    assert_eq!(s.turns().len(), 1, "有人看着、不是一次性的照旧都开");
    assert_eq!(replied(&s, &first), 2);
}
