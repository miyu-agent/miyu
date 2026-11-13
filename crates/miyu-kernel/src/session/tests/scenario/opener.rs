//! 场景：回合开始交给挂接点的 `opener` 在重启、崩溃前后（施工 LF-6，`docs/blueprint/kernel/session.md` 的
//! `RunTurnStartHooks`）。`Action` 不落盘：载入以后照日志重算，日志里没有这一格，靠的是这一轮 `turn.started` 的 `trigger` 和
//! 那条消息的 `by`。有计划的重启以后接着干的那一轮由 `turn.ended` 触发，没有（不再交一次「人生循环叫醒」，那一块在被打断的那
//! 一轮开头已经记进日志）；崩了收尾的那一轮不再问挂接点；载入以后再叫醒的照日志认得出。

use super::*;
use crate::origin::{Schedule, ScheduleLoop};

/// 人生循环交进来的那一句的 `by`。
fn life() -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: Some(ScheduleLoop::Life),
    })
}

#[test]
fn a_restart_picks_up_with_no_opener_and_a_reload_still_knows_the_loop() {
    let mut s = stage();
    s.model([Line::says("我先……").held(), Line::says("接着。")]);
    s.says_as(life(), "醒来，过你的日子。");
    assert_eq!(s.openers(), [Some(life())], "人生循环叫醒的一轮");

    let before = s.turns().len();
    s.restart();
    assert_eq!(s.turns().len(), before + 1, "接着干了：{:#?}", story(&s));
    assert_eq!(
        s.openers(),
        [Some(life()), None],
        "接着干的那一轮由 turn.ended 触发：没有 opener，不再当成人生循环叫醒"
    );

    s.model([Line::says("又醒了。")]);
    s.says_as(life(), "醒来，过你的日子。");
    assert_eq!(
        s.openers().last(),
        Some(&Some(life())),
        "载入以后照日志算，认得出人生循环"
    );
}

#[test]
fn a_crash_closes_the_turn_without_asking_the_hooks_again() {
    let mut s = stage();
    s.model([Line::says("我先……").held()]);
    s.says_as(life(), "醒来，过你的日子。");
    s.crash();
    assert_eq!(s.openers(), [Some(life())], "崩了收尾的那一轮不再问挂接点");

    s.model([Line::says("在。")]);
    s.say("在吗");
    assert!(
        matches!(s.openers(), [Some(_), Some(By::Person(_))]),
        "人开的一轮照常交人：{:?}",
        s.openers()
    );
}
