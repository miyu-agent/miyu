//! 场景：人敲 `/loop` 的第一次（施工 FX-14，`internal/blueprint/kernel/session.md`「定时交进来的话」第 3 条）。核心交
//! `Kickoff`：人亲手敲命令同人发一句话，还能恢复撤销也由它开一轮，开轮清掉撤销。同一句用 `Send` 交的照旧只记下。
//!
//! 带着人敲的那一行的（施工 FX-15）：记进 `message.user` 的 `command`；撤这一轮时同人说的话一起拿走，重做照原样重发；不带的
//! 照「定时交进来的话」留着。

use super::reports::last_request;
use super::*;
use crate::event::{Body, TypedCommand};
use crate::origin::{Schedule, ScheduleLoop};

/// 人敲的循环交进来的那一句的 `by`。
fn a_persons_loop() -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: Some(ScheduleLoop::Fixed),
        person: true,
        builtin: false,
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

/// 闲着的会话里交人敲的 `/loop 5m 看看 CI`：开了一轮，交回那一轮。
fn typed_kickoff(s: &mut Stage) -> TurnId {
    s.kicks_off_typed(a_persons_loop(), "看看 CI", "/loop 5m 看看 CI");
    *s.turns().last().expect("开了一轮")
}

/// 交进来的那一句在请求里的那一行（替身的组装一条事件一行，`Listing`）。
fn line_of_the_kickoff(s: &Stage) -> String {
    let seq = s
        .log()
        .iter()
        .find(|event| matches!(event.by, By::Schedule(_)))
        .expect("交进来了")
        .seq;
    format!("{} message.user\n", seq.get())
}

#[test]
fn a_typed_kickoff_records_the_line_the_person_typed() {
    let mut s = stage();
    s.model([Line::says("看过了。")]);
    typed_kickoff(&mut s);
    let message = s
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::MessageUser(message) => Some(message.clone()),
            _ => None,
        })
        .expect("记了那一句");
    assert_eq!(
        message.command,
        Some(TypedCommand {
            name: "loop".to_string(),
            text: "/loop 5m 看看 CI".to_string(),
        })
    );
    assert!(
        !last_request(&s).contains("/loop"),
        "人敲的那一行不进请求：{}",
        last_request(&s)
    );
}

#[test]
fn undoing_a_typed_kickoff_takes_the_line_away_like_the_persons_words() {
    let mut s = stage();
    s.model([Line::says("看过了。"), Line::says("在的。")]);
    let turn = typed_kickoff(&mut s);
    let said = line_of_the_kickoff(&s);
    assert!(
        last_request(&s).contains(&said),
        "撤之前在：{}",
        last_request(&s)
    );
    s.revert(turn);
    s.say("在吗");
    let last = last_request(&s);
    assert!(!last.contains(&said), "同人说的话一起撤：{last}");
}

#[test]
fn an_untyped_kickoff_stays_after_undo_like_other_scheduled_words() {
    let mut s = stage();
    s.model([Line::says("看过了。"), Line::says("在的。")]);
    s.kicks_off_as(a_persons_loop(), "看看 CI");
    let turn = *s.turns().last().expect("开了一轮");
    let said = line_of_the_kickoff(&s);
    s.revert(turn);
    s.say("在吗");
    assert!(
        last_request(&s).contains(&said),
        "定时交进来的照旧留着：{}",
        last_request(&s)
    );
}

#[test]
fn redoing_a_typed_kickoff_sends_it_again_as_it_was() {
    let mut s = stage();
    s.model([Line::says("看过了。"), Line::says("又看了。")]);
    typed_kickoff(&mut s);
    s.redo(None);
    assert_eq!(s.turns().len(), 2, "重做开了新的一轮：{:#?}", story(&s));
    let again: Vec<_> = s
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageUser(message) => Some((event.by.clone(), message.command.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(again.len(), 2, "{again:?}");
    assert_eq!(again[1].0, a_persons_loop(), "by 照原来的");
    assert_eq!(
        again[1].1.as_ref().map(|c| c.text.as_str()),
        Some("/loop 5m 看看 CI")
    );
}
