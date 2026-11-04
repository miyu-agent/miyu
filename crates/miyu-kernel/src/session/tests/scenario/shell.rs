//! 人用 `!` 打的命令（施工 SH-1 上，设计 57 第三节，`docs/blueprint/kernel/session.md`「人跑的命令」）：闲着收到就开一轮、
//! 第一件跑、记下再问；忙着排着、到两步之间先跑；几条照先后跑；退回的不跑；打断时在跑的叫停、记 `interrupted`、她不回；
//! 不接着发的没跑的也记 `interrupted`；重启时在跑的记 `aborted`、崩了的载入时补；撤销连同 `shell.ran` 一起撤；不是人的拒绝。

use super::*;
use crate::event::{ShellRan, ShellReason};
use crate::testkit::ShellPlay;

/// 记下的 `shell.ran`，照先后。
fn rans(s: &Stage) -> Vec<(Seq, ShellRan)> {
    s.log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ShellRan(ran) => Some((event.seq, ran.clone())),
            _ => None,
        })
        .collect()
}

/// 写了 `shell` 的 `message.user` 的序号，照先后。
fn shells(s: &Stage) -> Vec<Seq> {
    s.log()
        .iter()
        .filter(|event| matches!(&event.body, Body::MessageUser(message) if message.shell))
        .map(|event| event.seq)
        .collect()
}

#[test]
fn a_command_while_idle_opens_a_turn_runs_first_then_asks() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Exits(0, "clean\n".to_string())]);
    s.model([Line::says("工作区是干净的。")]);
    s.shell("git status");
    let message = shells(&s)[0];
    assert_eq!(
        s.shell_runs(),
        [(message, "git status".to_string(), "~/src/miyu".to_string())]
    );
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    let (at, ran) = &ran[0];
    assert_eq!(
        (ran.message, ran.exit, ran.reason.clone()),
        (message, Some(0), None)
    );
    assert_eq!(ran.text, "clean\n");
    let (seen, request) = s.requests().last().unwrap();
    assert!(seen >= at, "落了盘再问");
    assert!(listed_request(request).contains(&format!("{at} shell.ran\n")));
    assert_eq!(s.turns().len(), 1);
}

#[test]
fn commands_while_busy_queue_and_run_in_order_before_the_next_request() {
    let mut s = stage();
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("看看 a");
    let running = s.ran().last().unwrap().0;
    s.shell("ls");
    s.shell("pwd");
    assert!(s.shell_runs().is_empty(), "排着，还没听到");
    s.release_tool(running);
    let messages = shells(&s);
    let runs: Vec<_> = s
        .shell_runs()
        .iter()
        .map(|run| (run.0, run.1.as_str()))
        .collect();
    assert_eq!(runs, [(messages[0], "ls"), (messages[1], "pwd")], "照先后");
    let ran = rans(&s);
    let (seen, request) = s.requests().last().unwrap();
    assert!(*seen >= ran[1].0, "两条都落了盘再问");
    let listed = listed_request(request);
    assert!(listed.contains(&format!("{} shell.ran\n", ran[0].0)));
    assert_eq!(s.requests().len(), 2, "两步之间跑，不另开一轮");
    assert_eq!(s.turns().len(), 1);
}

#[test]
fn a_command_in_the_last_step_runs_in_the_turn_it_opens() {
    let mut s = stage();
    s.model([Line::says("好").held(), Line::says("看到了。")]);
    s.say("hi");
    s.shell("date");
    assert!(s.shell_runs().is_empty());
    s.release_model();
    assert_eq!(s.shell_runs().len(), 1);
    let turns = s.turns();
    assert_eq!(turns.len(), 2, "由它接着开一轮");
    let ran = rans(&s);
    assert_eq!(
        event_turn(&s, ran[0].0),
        Some(turns[1]),
        "在接着开的那一轮里跑"
    );
}

#[test]
fn withdrawn_commands_never_run() {
    let mut s = stage();
    s.model([Line::says("好").held()]);
    s.say("hi");
    s.shell("rm -rf build");
    s.withdraw(None);
    s.shell("touch x");
    s.interrupt(Queued::Return);
    assert!(s.shell_runs().is_empty(), "退回的不跑");
    assert!(rans(&s).is_empty(), "退回的不记 shell.ran");
}

#[test]
fn interrupting_a_running_command_halts_it_and_she_does_not_reply() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Holds]);
    s.hold_wakes();
    s.shell("sleep 100");
    let message = shells(&s)[0];
    let interrupt = s.interrupt(Queued::Send);
    assert_eq!(s.shell_halts(), [message]);
    assert!(
        matches!(s.outcome(&interrupt), Some(Outcome::Accepted { .. })),
        "当场回应"
    );
    assert!(rans(&s).is_empty(), "等它交回到这时的输出");
    s.finish_shell(None, Some(ShellReason::Interrupted), "partial\n");
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    assert_eq!(
        (ran[0].1.reason.clone(), ran[0].1.text.as_str()),
        (Some(ShellReason::Interrupted), "partial\n")
    );
    assert_eq!(
        story(&s).last().map(String::as_str),
        Some("7 turn.ended:interrupted person t3")
    );
    assert!(s.requests().is_empty(), "她不回");
}

#[test]
fn a_command_that_will_not_stop_is_given_up_when_the_wait_runs_out() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Holds]);
    s.hold_wakes();
    s.shell("sleep 100");
    s.interrupt(Queued::Send);
    s.release_wake();
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    assert_eq!(
        (ran[0].1.reason.clone(), ran[0].1.text.as_str()),
        (Some(ShellReason::Interrupted), "")
    );
    assert!(
        s.turns().is_empty()
            || story(&s)
                .iter()
                .any(|line| line.contains("turn.ended:interrupted"))
    );
    s.finish_shell(Some(0), None, "late\n");
    assert_eq!(rans(&s).len(), 1, "迟到的不理");
}

#[test]
fn queued_commands_not_sent_on_are_recorded_interrupted_without_running() {
    let mut s = stage();
    s.model([Line::says("好").held()]);
    s.say("hi");
    s.shell("make");
    s.interrupt(Queued::Keep);
    assert!(s.shell_runs().is_empty());
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].1.reason, Some(ShellReason::Interrupted));
    assert_eq!(ran[0].1.message, shells(&s)[0]);
}

#[test]
fn queued_commands_sent_on_run_in_the_next_turn() {
    let mut s = stage();
    s.model([Line::says("好").held(), Line::says("跑完了。")]);
    s.say("hi");
    s.shell("make");
    s.interrupt(Queued::Send);
    assert_eq!(s.shell_runs().len(), 1, "接着开的那一轮里跑");
    let ran = rans(&s);
    assert_eq!((ran.len(), ran[0].1.reason.clone()), (1, None));
}

#[test]
fn a_planned_restart_aborts_the_running_command_and_does_not_rerun_it() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Holds]);
    s.model([Line::says("重启以后接着。")]);
    s.shell("sleep 100");
    let message = shells(&s)[0];
    s.restart();
    assert_eq!(s.shell_halts(), [message]);
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].1.reason, Some(ShellReason::Aborted));
    assert_eq!(s.shell_runs().len(), 1, "接着干的那一轮不重跑");
}

#[test]
fn a_crash_while_running_fills_in_aborted_on_load() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Holds]);
    s.shell("sleep 100");
    s.crash();
    let ran = rans(&s);
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].1.reason, Some(ShellReason::Aborted));
    assert!(story(&s).last().unwrap().contains("turn.ended:aborted"));
    assert_eq!(s.shell_runs().len(), 1, "不重跑");
}

#[test]
fn undoing_the_turn_takes_the_command_and_its_output_with_it() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Exits(0, "v1\n".to_string())]);
    s.model([Line::says("看到了。"), Line::says("嗯。")]);
    s.shell("cat VERSION");
    let turn = s.turns()[0];
    s.revert(turn);
    s.say("别的");
    let (_, request) = s.requests().last().unwrap();
    let listed = listed_request(request);
    assert!(!listed.contains("shell.ran"), "{listed}");
    assert!(!listed.contains("message.user\n2"), "{listed}");
    assert_eq!(s.shell_runs().len(), 1);
}

#[test]
fn only_a_person_may_run_a_command_and_not_an_empty_one() {
    let mut s = stage();
    let harness = s.shell_as(
        By::Harness(crate::origin::Harness {
            name: crate::id::HarnessName::parse("bridge").unwrap(),
        }),
        "id",
    );
    assert_eq!(rejection(&s, &harness), "shell_not_allowed");
    let empty = s.shell("   ");
    assert_eq!(rejection(&s, &empty), "empty_message");
    assert!(s.log().len() == 1 && s.shell_runs().is_empty());
}

#[test]
fn her_reply_reads_the_command_and_its_output_together() {
    let mut s = stage();
    s.shell_plays([ShellPlay::Exits(2, "no such file\n".to_string())]);
    s.model([Line::says("文件不在。")]);
    s.shell("cat nope");
    let ran = rans(&s);
    assert_eq!(ran[0].1.exit, Some(2));
    assert_eq!(ran[0].1.text, "no such file\n");
}

/// 第 `seq` 条的回合编号。
fn event_turn(s: &Stage, seq: Seq) -> Option<TurnId> {
    s.log()
        .iter()
        .find(|event| event.seq == seq)
        .and_then(|event| event.turn)
}

/// 命令 `id` 被拒的原因码。
fn rejection(s: &Stage, id: &CommandId) -> &'static str {
    match s.outcome(id) {
        Some(Outcome::Rejected { reason, .. }) => reason.code(),
        other => panic!("{id} 没被拒：{other:?}"),
    }
}
