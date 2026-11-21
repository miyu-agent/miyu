//! 场景：人敲 `/loop` 的第一次（施工 FX-14，`internal/blueprint/kernel/session.md`「定时交进来的话」第 3 条）。核心交
//! `Kickoff`：人亲手敲命令同人发一句话，还能恢复撤销也由它开一轮，开轮清掉撤销。同一句用 `Send` 交的照旧只记下。

use super::*;
use crate::origin::{Schedule, ScheduleLoop};

/// 人敲的循环交进来的那一句的 `by`。
fn a_persons_loop() -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: Some(ScheduleLoop::Fixed),
        person: true,
    })
}

/// 说过两轮、撤掉第二轮：撤销还没处理。
fn undone(s: &mut Stage) {
    s.say("hi");
    s.say("再来");
    let second = s.turns()[1];
    s.revert(second);
}

#[test]
fn a_kickoff_while_an_undo_can_be_restored_opens_a_turn_and_settles_it() {
    let mut s = stage();
    s.model([
        Line::says("好。"),
        Line::says("嗯。"),
        Line::says("看过了。"),
    ]);
    undone(&mut s);
    let id = s.kicks_off_as(a_persons_loop(), "看看 CI");
    assert_eq!(s.turns().len(), 3, "由它开一轮：{:#?}", story(&s));
    match s.outcome(&id) {
        Some(Outcome::Accepted { events }) => assert_eq!(events.len(), 2, "附上开的那一轮"),
        other => panic!("没接受：{other:?}"),
    }
    let restored = s.unrevert();
    assert_eq!(
        s.outcome(&restored),
        Some(&Outcome::Rejected {
            reason: Reason::NothingToUnrevert,
            about: Vec::new(),
        }),
        "开了轮，撤销清掉了"
    );
}

#[test]
fn the_same_words_sent_as_a_message_are_only_recorded() {
    let mut s = stage();
    s.model([Line::says("好。"), Line::says("嗯。")]);
    undone(&mut s);
    s.says_as(a_persons_loop(), "看看 CI");
    assert_eq!(s.turns().len(), 2, "`Send` 照旧只记下：{:#?}", story(&s));
}

#[test]
fn nobody_watching_a_kickoff_is_only_recorded() {
    let mut s = Stage::oneshot(policy, environment("~/src/miyu"), at(0));
    s.kicks_off_as(a_persons_loop(), "看看 CI");
    assert!(s.turns().is_empty(), "没人看着的一次性会话里循环照旧只记下");
}
