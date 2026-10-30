//! 头自己数还有几个任务没报（施工 7-9；施工 AP-4 起后台命令也数）：派出去、回报、留言，叫醒的那一轮来不来。

use serde_json::{Value, json};

use super::*;

/// 调用 `call` 的结果，带这些效果。
fn result(call: &str, effects: Value) -> Value {
    json!({"call_id": call, "status": "ok", "blocks": [], "effects": effects})
}

/// 派出去一个子代理 `job`。
fn started(job: &str, title: &str) -> Value {
    json!({"kind": "job.started", "job": job, "what": "agent", "title": title, "session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"})
}

/// 第 `seq` 条：子代理 `job` 已回报，原因 `reason`。
fn report(seq: u64, job: &str, reason: &str) -> Value {
    json!({"seq": seq, "kind": "child.reported", "body": {"job": job, "reason": reason, "text": ""}})
}

/// 第 `seq` 条：后台命令 `job` 结束了，原因 `reason`（施工 AP-4）。
fn ended(seq: u64, job: &str, reason: &str) -> Value {
    json!({"seq": seq, "kind": "job.reported", "body": {"job": job, "reason": reason}})
}

/// 报回来的是 `job`，哪一种、标题。
fn back(job: &str, kind: Kind, title: Option<&str>) -> Option<Reported> {
    Some(Reported {
        job: job.to_string(),
        kind,
        title: title.map(str::to_string),
    })
}

/// 欠着 `agents` 个子代理、`commands` 个后台命令的回报。
fn owing(agents: usize, commands: usize) -> Owed {
    Owed { agents, commands }
}

/// 派了 j1、j2 两个子代理。
fn two() -> Jobs {
    let mut agents = Jobs::default();
    agents.result(&result(
        "call_10_1",
        json!([started("j1", "查 A"), started("j2", "查 B")]),
    ));
    agents
}

#[test]
fn subagents_and_commands_are_owed_until_they_report() {
    // 施工 AP-4（2026-10-31 项目主人定）：后台命令也等，以前不数。同一次调用里派了两个子代理，还放了一条后台命令 j3。
    let mut agents = Jobs::default();
    agents.result(&result(
        "call_10_1",
        json!([started("j1", "查 A"), started("j2", "查 B"),
            {"kind": "job.started", "job": "j3", "what": "command", "title": "跑测试"}]),
    ));
    assert_eq!(agents.owed(), owing(2, 1), "后台命令也等");
    assert!(!agents.settled());
    assert_eq!(
        agents.reported(&report(20, "j1", "done"), true),
        back("j1", Kind::Agent, Some("查 A"))
    );
    assert_eq!(agents.owed(), owing(1, 1));
    // 停掉的、崩了补报的也算报过。
    agents.reported(&report(21, "j2", "aborted"), true);
    assert_eq!(agents.owed(), owing(0, 1));
    assert!(!agents.settled(), "还欠后台命令那一份");
    assert_eq!(
        agents.reported(&ended(22, "j3", "exited"), true),
        back("j3", Kind::Command, Some("跑测试")),
        "后台命令的回报是 job.reported"
    );
    assert_eq!(agents.owed(), owing(0, 0));
    assert_eq!(
        agents.reported(&report(23, "j9", "done"), false),
        None,
        "不是这次派出去的：不认，也不等它叫醒的那一轮"
    );
    assert!(agents.settled());
}

#[test]
fn a_command_from_before_is_not_waited_for() {
    // 这一次以前放到后台的命令：看不到放它的那一条，它的回报不认、不数，也不等它叫醒的那一轮。
    let mut agents = Jobs::default();
    assert_eq!(agents.reported(&ended(20, "j4", "exited"), false), None);
    assert!(agents.settled());
    // 别的种类的任务（不是子代理、不是后台命令）、别的事件：不等。
    agents.result(&result(
        "call_10_1",
        json!([{"kind": "job.started", "job": "j5", "what": "someday", "title": "?"}]),
    ));
    assert_eq!(agents.owed(), owing(0, 0));
    assert_eq!(
        agents.reported(
            &json!({"seq": 21, "kind": "turn.ended", "body": {"job": "j1"}}),
            false
        ),
        None
    );
}

#[test]
fn a_message_after_its_report_makes_it_owe_again() {
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), false);
    agents.turn_started();
    // 第 30 条回复里的调用留了言：回报在它前面，又欠一份。
    agents.result(&result(
        "call_30_1",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), owing(2, 0));
    // 留言发出去以后、结果记下以前它就报了：算回了这句留言（内核的账本一样算）。
    agents.reported(&report(35, "j1", "done"), true);
    agents.result(&result(
        "call_30_2",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), owing(1, 0), "只剩 j2");
    // 给这一次以前派的留言：也等它（施工 7-9 补）。
    agents.result(&result(
        "call_40_1",
        json!([{"kind": "job.messaged", "job": "j7"}]),
    ));
    assert_eq!(agents.owed(), owing(2, 0), "j2、j7");
    // 调用编号读不出来的：当刚发，欠着。
    agents.result(&result(
        "bad",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), owing(3, 0));
}

#[test]
fn a_report_while_she_is_idle_brings_a_turn() {
    // 照内核的规矩（`kernel/session.md`「回报」第 5 条）：子代理、后台命令各自哪几种原因叫醒她。
    for (what, reason, by_model, wakes) in [
        ("agent", "done", false, true),
        ("agent", "stopped", false, true),
        ("agent", "stopped", true, false),
        ("agent", "undone", false, false),
        ("agent", "aborted", false, false),
        ("agent", "someday", false, true),
        ("agent", "restarted", false, true),
        ("command", "exited", false, true),
        ("command", "timed_out", false, true),
        ("command", "too_large", false, true),
        ("command", "stopped", false, true),
        ("command", "stopped", true, false),
        ("command", "undone", false, false),
        ("command", "aborted", false, false),
        ("command", "restarted", false, false),
        ("command", "someday", false, true),
    ] {
        let mut agents = Jobs::default();
        let started = json!([{"kind": "job.started", "job": "j1", "what": what, "title": "查 A"}]);
        agents.result(&result("call_10_1", started));
        let mut event = match what {
            "agent" => report(20, "j1", reason),
            _ => ended(20, "j1", reason),
        };
        event["body"]["by_model"] = json!(by_model);
        agents.reported(&event, false);
        assert_eq!(agents.settled(), !wakes, "{what} {reason} {by_model}");
        agents.turn_started();
        assert!(agents.settled(), "等的那一轮开了头");
    }
}

#[test]
fn a_report_the_last_request_missed_brings_the_next_turn() {
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.reported(&report(21, "j2", "done"), true);
    // 最后一次请求看到了第 20 条：j2 那一条没听到，内核接着开一轮。
    agents.called(&json!({"seen": 20}));
    agents.turn_ended("completed");
    assert!(!agents.settled());
    // 看的是最大的那个：压缩的摘要请求看到的在前面，不算没听到。
    let mut heard = two();
    heard.reported(&report(20, "j1", "done"), true);
    heard.reported(&report(21, "j2", "done"), true);
    heard.called(&json!({"seen": 21}));
    heard.called(&json!({"seen": 12}));
    heard.turn_ended("completed");
    assert!(heard.settled());
    agents.turn_started();
    agents.called(&json!({"seen": 25}));
    agents.turn_ended("error");
    assert!(agents.settled(), "下一轮里没有新到的：了结了");
    // 打断结束的不接着开。
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.reported(&report(21, "j2", "done"), true);
    agents.turn_ended("interrupted");
    assert!(agents.settled());
    // 她自己停的：不排进这一轮的回报队。
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.called(&json!({"seen": 20}));
    let mut stopped = report(21, "j2", "stopped");
    stopped["body"]["by_model"] = json!(true);
    agents.reported(&stopped, true);
    agents.turn_ended("completed");
    assert!(agents.settled());
}

#[test]
fn a_subagent_from_before_is_owed_once_she_messages_it() {
    // 施工 7-9 补：这一次以前派的 j5，没留言以前它的回报不认、不数；这一次给它留了言，也等它。
    let mut agents = Jobs::default();
    assert_eq!(
        agents.reported(&report(20, "j5", "done"), true),
        None,
        "还没留言：不认"
    );
    assert!(agents.settled());
    agents.result(&result(
        "call_30_1",
        json!([{"kind": "job.messaged", "job": "j5"}]),
    ));
    assert_eq!(
        agents.owed(),
        owing(1, 0),
        "留言以前到的回报不算回了这句留言"
    );
    assert_eq!(
        agents.reported(&report(40, "j5", "done"), false),
        back("j5", Kind::Agent, None),
        "留了言以后的回报认，没有标题"
    );
    assert_eq!(agents.owed(), owing(0, 0));
    assert!(!agents.settled(), "等它叫醒的那一轮");
    // 留言发出去以后、结果记下以前它就报了：和这一次派的一样，算回了这句留言，不欠。
    let mut quick = Jobs::default();
    quick.reported(&report(35, "j6", "done"), true);
    quick.result(&result(
        "call_30_1",
        json!([{"kind": "job.messaged", "job": "j6"}]),
    ));
    assert_eq!(quick.owed(), owing(0, 0), "留言发出以后先到的回报算回了");
    // 没留言的以前派的：一直不数。
    quick.reported(&report(50, "j7", "done"), false);
    assert!(quick.settled(), "不等它叫醒的那一轮");
}
