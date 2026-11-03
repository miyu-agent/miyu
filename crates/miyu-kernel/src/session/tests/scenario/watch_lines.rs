//! 监视的命令出的行（施工 MN-1 上，设计 56 第三节，`docs/blueprint/kernel/session.md`「回报」）：闲着时一批记一条、开一轮；
//! 她这一轮在跑时挂在这一轮上，下一步问模型以前合成一条记下；这一轮没有下一步的回合结束时记下、接着开；截到 4000 字；最近
//! 一小时记满 120 条再来的不记、发一次停；撤销不删；没人看着的一次性会话只记下；派它的那一轮撤掉了的不叫醒。

use super::reports::{dispatched, event, last_request, starter, trigger_of};
use super::*;
use crate::event::{JobLines, JobReason};
use crate::session::{Action, CHATTY, LINE_CHARS, StopReason};

/// 第 `n` 条是 `job.lines`，交回它。
fn lines_at(s: &Stage, n: u64) -> &JobLines {
    match &event(s, n).body {
        Body::JobLines(lines) => lines,
        body => panic!("第 {n} 条应该是 job.lines：{body:?}"),
    }
}

/// 记下的 `job.lines` 一共几条。
fn recorded(s: &Stage) -> usize {
    s.log()
        .iter()
        .filter(|event| matches!(event.body, Body::JobLines(_)))
        .count()
}

#[test]
fn a_batch_while_idle_opens_a_turn_it_triggers() {
    let mut s = dispatched(stage());
    s.model([Line::says("看到报错了。")]);
    s.job_lines(2, "ERROR disk full", 1, starter(), Some(id(1)));
    assert_eq!(
        story(&s)[12..],
        [
            "13 job.lines tool call_6_2",
            "14 turn.started kernel t14",
            "15 message.assistant model t14",
            "16 model.called:ok kernel t14",
            "17 turn.ended:completed kernel t14",
        ]
    );
    assert_eq!(trigger_of(&s, 14), Some(seq(13)));
    assert_eq!(event(&s, 13).turn, None, "不带回合编号");
    assert_eq!(event(&s, 13).cause, Some(id(1)));
    assert!(last_request(&s).ends_with("13 job.lines\n14 turn.started\n"));
}

#[test]
fn batches_while_busy_become_one_before_the_next_request() {
    let mut s = dispatched(stage());
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("再看看 a");
    let running = s.ran().last().unwrap().0;
    s.job_lines(2, "one", 1, starter(), Some(id(1)));
    s.job_lines(2, "two\nthree", 2, starter(), Some(id(1)));
    assert_eq!(recorded(&s), 0, "她在跑：挂着，还没记");
    s.release_tool(running);
    assert_eq!(recorded(&s), 1, "合成一条");
    let lines = s
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::JobLines(lines) => Some((event.seq, lines.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(lines.1.text, "one\ntwo\nthree");
    assert_eq!((lines.1.lines, lines.1.omitted), (3, 0));
    let (seen, request) = s.requests().last().unwrap();
    assert!(*seen >= lines.0, "落了盘再问");
    assert!(
        listed_request(request).contains(&format!("{} job.lines\n", lines.0)),
        "下一次请求就有它"
    );
    assert_eq!(s.turns().len(), 2, "下一步听到了，结束时不再开");
}

#[test]
fn a_batch_in_the_last_step_is_recorded_when_the_turn_ends_and_opens_the_next() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("看到输出了。")]);
    s.say("hi");
    s.job_lines(2, "ERROR", 1, starter(), Some(id(1)));
    assert_eq!(recorded(&s), 0);
    s.release_model();
    let at = s
        .log()
        .iter()
        .position(|event| matches!(event.body, Body::JobLines(_)))
        .expect("回合结束时记下了");
    let seq_of_lines = s.log()[at].seq;
    let next = s
        .log()
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .unwrap()
        .seq;
    assert_eq!(trigger_of(&s, next.get()), Some(seq_of_lines), "由它接着开");
}

#[test]
fn merged_batches_are_cut_to_the_limit_keeping_head_and_tail() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("看到了。")]);
    s.say("hi");
    let head = "a".repeat(3000);
    let tail = "b".repeat(3000);
    s.job_lines(2, &head, 1, starter(), None);
    s.job_lines(2, &tail, 1, starter(), None);
    s.release_model();
    let n = s
        .log()
        .iter()
        .find(|event| matches!(event.body, Body::JobLines(_)))
        .unwrap()
        .seq
        .get();
    let lines = lines_at(&s, n);
    let total = 3000 + 1 + 3000;
    assert_eq!(lines.omitted, u64::try_from(total - LINE_CHARS).unwrap());
    assert!(lines.text.starts_with(&"a".repeat(LINE_CHARS / 2)));
    assert!(lines.text.ends_with(&"b".repeat(LINE_CHARS / 2)));
    // 替身的那一行省略是 `[{count} cut]`。
    assert!(
        lines
            .text
            .contains(&format!("[{} cut]", total - LINE_CHARS)),
        "中间一行写省了多少"
    );
}

#[test]
fn a_chatty_watch_is_stopped_once_after_an_hour_holds_the_limit() {
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    for n in 0..CHATTY {
        s.job_lines(2, &format!("line {n}"), 1, starter(), Some(id(1)));
    }
    assert_eq!(recorded(&s), CHATTY, "没人看着：一批一条，只记下");
    assert!(s.stopping().is_empty(), "记满了还没多出来");
    s.job_lines(2, "one too many", 1, starter(), Some(id(1)));
    assert_eq!(recorded(&s), CHATTY, "多出来的不记");
    assert_eq!(
        s.stopping(),
        [Action::StopJobs {
            jobs: vec![crate::id::JobId::new(2).unwrap()],
            by: starter(),
            cause: Some(id(1)),
            reason: StopReason::TooChatty,
        }]
    );
    s.job_lines(2, "and another", 1, starter(), Some(id(1)));
    assert_eq!(s.stopping().len(), 1, "只发一次");
}

#[test]
fn lines_older_than_an_hour_do_not_count() {
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    for n in 0..CHATTY - 1 {
        s.job_lines(2, &format!("line {n}"), 1, starter(), None);
    }
    s.advance(61);
    s.job_lines(2, "an hour later", 1, starter(), None);
    s.job_lines(2, "still fine", 1, starter(), None);
    assert!(s.stopping().is_empty(), "滚动的一小时：以前的不算");
    assert_eq!(recorded(&s), CHATTY + 1);
}

#[test]
fn undoing_the_turn_it_opened_keeps_it() {
    let mut s = dispatched(stage());
    s.model([Line::says("看到报错了。")]);
    s.job_lines(2, "ERROR", 1, starter(), Some(id(1)));
    assert_eq!(trigger_of(&s, 14), Some(seq(13)));
    s.revert(TurnId::new(seq(14)));
    assert!(
        matches!(event(&s, 13).body, Body::JobLines(_)),
        "日志里照留着"
    );
    s.model([Line::says("还在。")]);
    s.say("刚才那行呢");
    assert!(
        last_request(&s).contains("13 job.lines\n"),
        "撤销那一轮不拿走别处来的：{}",
        last_request(&s)
    );
}

#[test]
fn lines_of_a_watch_dispatched_in_an_undone_turn_do_not_wake_her() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    let turns = s.turns().len();
    s.job_lines(2, "ERROR", 1, starter(), None);
    assert_eq!(s.turns().len(), turns, "派它的那一轮撤掉了：只记下");
}

#[test]
fn lines_of_a_command_that_ended_are_ignored() {
    let mut s = dispatched(stage());
    s.model([Line::says("结束了。")]);
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    let length = s.log().len();
    s.job_lines(2, "late", 1, starter(), None);
    assert_eq!(s.log().len(), length, "结束过的不理");
}
