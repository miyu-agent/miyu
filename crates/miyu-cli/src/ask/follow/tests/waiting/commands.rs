//! 后台命令也等（施工 AP-4，2026-10-31 项目主人定，`docs/blueprint/cli/ask.md`「等子代理」）：和子代理一起数，等的
//! 那一行照实写几样；报回来了那一行同子代理；不等了、到时间了那一句说是哪几样在跑；事件流写一行 `job.reported`。

use serde_json::{Value, json};

use super::*;

/// 放到后台一条命令 `job`。
fn command(job: &str, title: &str) -> Value {
    json!({"kind": "job.started", "job": job, "what": "command", "title": title})
}

/// 第 `seq` 条：后台命令 `job` 结束了，原因 `reason`，退出码 `exit`（没有的不写）。
fn ended(seq: u64, job: &str, reason: &str, exit: Option<i32>) -> Value {
    let mut body = json!({"job": job, "reason": reason, "duration_ms": 1200});
    if let Some(exit) = exit {
        body["exit_code"] = json!(exit);
    }
    at(seq, "job.reported", 0, "cmd-6f92", body)
}

/// 第一轮放到后台 j3「跑测试」，说「放到后台了。」；`agents` 是同一次调用里还派出去 j1「查 A」。
fn first_with_command(agents: bool) -> Vec<Value> {
    let mut effects = vec![command("j3", "跑测试")];
    if agents {
        effects.insert(0, agent("j1", "查 A"));
    }
    round(
        10,
        "ask-4",
        "放到后台了。",
        12,
        Value::Array(effects),
        "completed",
    )
}

#[test]
fn a_subagent_and_a_command_are_both_waited_for() {
    let plan = plan(Output::Text, Language::Chinese);
    let mut messages = first_with_command(true);
    messages.push(report(20, "j1", "done"));
    messages.extend(round(
        21,
        "child/report/7",
        "A 好了。",
        25,
        json!([]),
        "completed",
    ));
    let (step, _, _) = waiting(&plan, false, &messages, |follow, _| {
        assert!(follow.waiting(), "子代理报了，后台命令还没报：接着等");
        None
    });
    assert_eq!(step, Step::Going);
    messages.push(ended(30, "j3", "exited", Some(0)));
    messages.extend(round(
        31,
        "cmd-6f92",
        "都好了。",
        36,
        json!([]),
        "completed",
    ));
    let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert_eq!(step, Step::Done(exit::OK));
    assert!(
        screen.ends_with(
            "\n\n放到后台了。\n· 等待 1 个子代理、1 个后台命令回报…（按 Ctrl+C 停止等待）\n\
             · j1「查 A」已回报\n\nA 好了。\n· j3「跑测试」已回报\n\n都好了。\n\
             · 输入 300 · 命中缓存 120（40%）· 输出 30\n"
        ),
        "{screen}"
    );
}

#[test]
fn a_command_that_wakes_nobody_ends_it_at_once() {
    // 有计划的重启停掉的、她自己停掉的：只记下、不叫醒她，没有要来的一轮，当场收尾。
    let plan = plan(Output::Text, Language::Chinese);
    let mut stopped = ended(20, "j3", "stopped", None);
    stopped["params"]["event"]["body"]["by_model"] = json!(true);
    for last in [ended(20, "j3", "restarted", None), stopped] {
        let mut messages = first_with_command(false);
        messages.push(last);
        let (step, screen, _) = waiting(&plan, true, &messages, |_, _| None);
        assert_eq!(step, Step::Done(exit::OK));
        assert!(
            screen.ends_with(
                "\r\x1b[2K· 等待 1 个后台命令回报…（按 Ctrl+C 停止等待）\r\x1b[2K· j3「跑测试」已回报\n\
                 · 输入 100 · 命中缓存 40（40%）· 输出 10\n"
            ),
            "{screen:?}"
        );
    }
}

#[test]
fn leaving_while_a_command_runs_says_what_keeps_running() {
    let plan = plan(Output::Text, Language::Chinese);
    for (agents, why, said) in [
        (
            false,
            Leaving::TimedOut,
            "· 已到等待时限，未回报的后台命令仍在运行\n",
        ),
        (
            false,
            Leaving::Pressed,
            "· 已停止等待，后台命令仍在运行，下次 miyu ask -c 时会收到结果\n",
        ),
        (
            true,
            Leaving::TimedOut,
            "· 已到等待时限，未回报的子代理和后台命令仍在后台运行\n",
        ),
    ] {
        let (step, screen, left) = waiting(
            &plan,
            false,
            &first_with_command(agents),
            |follow, screen| {
                assert!(follow.waiting(), "第一轮结束了、命令还没报：在等");
                Some(follow.leave(why, screen))
            },
        );
        assert_eq!((step, left), (Step::Going, Some(exit::INTERRUPTED)));
        assert!(screen.ends_with(said), "{screen}");
    }
}

#[test]
fn english_counts_both_kinds() {
    let language = Language::English;
    assert_eq!(
        language.waiting(1, 2),
        "· Waiting for 1 subagent and 2 background commands to report… (Ctrl+C stops waiting)"
    );
    assert_eq!(
        language.waiting(0, 1),
        "· Waiting for 1 background command to report… (Ctrl+C stops waiting)"
    );
    assert_eq!(
        language.stopped_waiting(0, 1),
        "· Stopped waiting; the background commands keep running, and their results arrive at the next miyu ask -c"
    );
    assert_eq!(
        language.timed_out(2, 1),
        "· Time is up; the subagents and background commands that have not reported keep running"
    );
    assert_eq!(
        Language::Chinese.waiting(0, 2),
        "· 等待 2 个后台命令回报…（按 Ctrl+C 停止等待）"
    );
}

#[test]
fn jsonl_writes_a_line_when_a_command_reports() {
    // 有退出码的带 `exit`；被停掉的没有退出码，不写这一格。这一次以前放的命令的回报不写。
    let plan = plan(Output::Jsonl, Language::Chinese);
    let mut messages = first_with_command(false);
    messages.push(ended(15, "j8", "exited", Some(0)));
    messages.push(ended(20, "j3", "exited", Some(2)));
    let (_, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    let reported: Vec<Value> = screen
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["type"] == "job.reported")
        .collect();
    assert_eq!(
        reported,
        [json!({"exit": 2, "job": "j3", "reason": "exited", "type": "job.reported"})]
    );
    let mut messages = first_with_command(false);
    messages.push(ended(20, "j3", "stopped", None));
    let (_, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert!(
        screen.contains("{\"job\":\"j3\",\"reason\":\"stopped\",\"type\":\"job.reported\"}\n"),
        "{screen}"
    );
}
