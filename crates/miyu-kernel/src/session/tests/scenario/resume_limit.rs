//! 场景：被重启打断以后接着干，从人最后说的那句话起最多接 `resumes`（测试的策略是 3）次（施工 RS-1，
//! `docs/blueprint/kernel/session.md`「载入和崩溃」第 6 条）。只有人开口清零，回合中途插进来的也算；回报叫醒的一轮不清零：
//! 原来它一开就从 0 数，接着干的那一轮放一条后台命令去重启、自己结束，回报叫醒的下一轮被重启打断，又从 0 数，一直转下去。
//! 崩了照旧不接（`stopping.rs`），手动压缩那一轮照旧不接（`manual.rs`）。

use super::reports::starter;
use super::*;
use crate::event::{EndReason, JobReason};

/// 最后一条 `turn.ended` 的原因。
fn last_end(s: &Stage) -> Option<EndReason> {
    s.log().iter().rev().find_map(|event| match &event.body {
        Body::TurnEnded(ended) => Some(ended.reason.clone()),
        _ => None,
    })
}

/// 再重启一次：被打断的那一轮接着干了没有（多开了一轮）。
fn restart_resumes(s: &mut Stage) -> bool {
    let before = s.turns().len();
    s.restart();
    s.turns().len() > before
}

/// 人说一句，模型那一次停在路上：这一轮在跑。
fn running_after_saying(s: &mut Stage, words: &str) {
    s.model([Line::says("我先……").held()]);
    s.say(words);
}

#[test]
fn once_restarted_it_picks_up() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下");
    s.model([Line::says("接着。")]);
    assert!(restart_resumes(&mut s), "{:#?}", story(&s));
    assert_eq!(last_end(&s), Some(EndReason::Completed));
}

#[test]
fn after_three_pick_ups_the_fourth_restart_waits_for_a_person() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下");
    s.model(["一。", "二。", "三。"].map(|said| Line::says(said).held()));
    for n in 1..=3 {
        assert!(restart_resumes(&mut s), "第 {n} 次接着干：{:#?}", story(&s));
    }
    assert!(!restart_resumes(&mut s), "接够了三次：{:#?}", story(&s));
    assert_eq!(last_end(&s), Some(EndReason::Restarted), "什么都不另记");
}

/// 主力：接着干的那一轮放一条后台命令去重启、自己结束；命令的回报叫醒下一轮，那一轮又被重启打断。回报叫醒的一轮不清零，
/// 接够了三次，第四次照样不接。
#[test]
fn a_turn_woken_by_a_report_in_between_does_not_reset_the_count() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下，装好了重启核心");
    for job in 1..=3 {
        s.model([
            Line::calls("放一条命令去重启。", &[("shell", "{}")]),
            Line::says("等它重启。"),
            Line::says("重启好了吗……").held(),
        ]);
        s.tools([Play::starts_command(job, "重启核心")]);
        assert!(
            restart_resumes(&mut s),
            "第 {job} 次接着干：{:#?}",
            story(&s)
        );
        assert_eq!(last_end(&s), Some(EndReason::Completed));
        let before = s.turns().len();
        s.job_ends(job, JobReason::Exited, starter(), None);
        assert_eq!(
            s.turns().len(),
            before + 1,
            "回报叫醒了一轮：{:#?}",
            story(&s)
        );
    }
    assert!(
        !restart_resumes(&mut s),
        "人没开过口，接够了三次：{:#?}",
        story(&s)
    );
}

#[test]
fn a_person_speaking_starts_the_count_again() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下");
    s.model(["一。", "二。", "三。"].map(|said| Line::says(said).held()));
    for _ in 1..=3 {
        assert!(restart_resumes(&mut s));
    }
    assert!(!restart_resumes(&mut s));
    running_after_saying(&mut s, "接着弄");
    s.model(["四。", "五。", "六。"].map(|said| Line::says(said).held()));
    for n in 1..=3 {
        assert!(
            restart_resumes(&mut s),
            "人开口以后第 {n} 次：{:#?}",
            story(&s)
        );
    }
    assert!(!restart_resumes(&mut s), "{:#?}", story(&s));
}

#[test]
fn a_word_put_in_mid_turn_starts_the_count_again() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下");
    s.model(["一。", "二。"].map(|said| Line::says(said).held()));
    for _ in 1..=2 {
        assert!(restart_resumes(&mut s));
    }
    // 接着干的第二轮还在跑，人插进来一句：排着队，从这里重新数。
    s.say("顺便看看日志");
    s.model(["三。", "四。", "五。"].map(|said| Line::says(said).held()));
    for n in 1..=3 {
        assert!(
            restart_resumes(&mut s),
            "插话以后第 {n} 次：{:#?}",
            story(&s)
        );
    }
    assert!(!restart_resumes(&mut s), "{:#?}", story(&s));
}

#[test]
fn a_crash_after_a_pick_up_still_waits_for_a_person() {
    let mut s = stage();
    running_after_saying(&mut s, "升级一下");
    s.model([Line::says("一。").held()]);
    assert!(restart_resumes(&mut s));
    let before = s.turns().len();
    s.crash();
    assert_eq!(s.turns().len(), before, "崩了不接：{:#?}", story(&s));
    assert_eq!(last_end(&s), Some(EndReason::Aborted));
}
