//! 请求形状探针：人敲 `/loop` 当场开的第一轮（施工 FX-15，`internal/blueprint/schedules.md` 第五节第 9 条）。那句 `message.user`
//! 带着人敲的那一行（`command`），`by` 写了 `builtin`：都是给头看的，给模型的请求一个字节都不变。同一份剧本里换成不带的
//! `Kickoff`、不写 `builtin` 的 `by` 比，每一次请求逐字节相同；之后撤这一轮、人再说一句，撤掉的那句同人说的话一起不见。

use crate::support::{check, sent, stage};
use miyu_kernel::id::TurnId;
use miyu_kernel::origin::{By, Schedule, ScheduleLoop};
use miyu_kernel::testkit::{Line, Stage};

/// 人敲的循环交进来的那一句的 `by`：`builtin` 照 `builtin` 写。
fn by(builtin: bool) -> By {
    By::Schedule(Schedule {
        schedule: "t1".to_string(),
        looping: Some(ScheduleLoop::Dynamic),
        person: true,
        builtin,
    })
}

/// 剧本：人问一句；人敲 `/loop`（`typed` 的带上那一行、写 `builtin`），开了一轮；人再问一句。
fn script(typed: bool) -> Stage {
    let mut s = stage();
    s.model([Line::says("好，我看看。")]);
    s.say("CI 为什么红了？");
    s.advance(2);
    s.model([Line::says("接着看了。")]);
    match typed {
        true => s.kicks_off_typed(by(true), "Keep going on your own.", "/loop"),
        false => s.kicks_off_as(by(false), "Keep going on your own."),
    };
    s.advance(2);
    s.model([Line::says("还在看。")]);
    s.say("那测试呢？");
    s
}

#[test]
fn the_typed_line_and_builtin_do_not_reach_the_model() {
    let (typed, plain) = (script(true), script(false));
    assert_eq!(typed.requests().len(), 3);
    assert_eq!(typed.requests(), plain.requests(), "逐字节相同");
    if let Err(why) = check(&sent(&typed)) {
        panic!("{why}");
    }
}

#[test]
fn undoing_the_typed_round_takes_its_words_away() {
    let mut s = stage();
    s.model([Line::says("好。")]);
    s.say("在吗");
    s.advance(2);
    s.model([Line::says("接着看了。"), Line::says("在的。")]);
    s.kicks_off_typed(by(false), "看看 CI", "/loop 看看 CI");
    let turn: TurnId = *s.turns().last().expect("开了一轮");
    s.revert(turn);
    s.say("还在吗");
    let last = &s.requests().last().expect("发了").1;
    let text = format!("{:?}", last.messages);
    assert!(!text.contains("看看 CI"), "{text}");
}
