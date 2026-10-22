//! 会话状态里投影算得出的那一半（施工 9-8 补上）：闲着、在跑、在等人；正在做什么；上下文用了多少；重试和都在冷却；任务表
//! 照派出、了结、叫醒记，做完的只留最近的几个。

use std::sync::Arc;

use miyu_kernel::event::{
    Choice, Decision, ErrorClass, JobReason, Question, Response, TransientBody, Usage,
};
use miyu_kernel::id::ModuleId;
use miyu_kernel::origin::By;
use miyu_kernel::session::Verdict;
use miyu_kernel::testkit::{Line, Play, Stage, Streamed};
use miyu_kernel::tool::Access;
use miyu_view::{Doing, JobState, Projector, State, Status, Wait};

use crate::support::*;

/// 落了盘的和瞬时的照先后喂，每喂一条记一份状态。
fn statuses(stage: &Stage) -> Vec<Status> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    let mut seen = Vec::new();
    for item in stage.stream() {
        match item {
            Streamed::Event(event) => drop(projector.event(event)),
            Streamed::Transient(transient) => drop(projector.transient(transient)),
        }
        seen.push(projector.status());
    }
    seen
}

/// 最后的状态：只喂落了盘的（翻页、订阅时那一页的样子）。
fn paged(stage: &Stage) -> Status {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        drop(projector.event(event));
    }
    projector.status()
}

#[test]
fn a_finished_turn_is_idle_with_the_context_it_used() {
    let mut stage = stage();
    stage.model([Line::says("Hi.").reports(1_234)]);
    stage.say("hello");
    let status = paged(&stage);
    assert_eq!(status.state, State::Idle);
    assert_eq!(status.since, None);
    assert_eq!(status.doing, None);
    assert_eq!(status.used, Some(1_234));
    let live = statuses(&stage);
    assert!(
        live.iter()
            .any(|s| s.state == State::Running && s.since.is_some()),
        "跑的时候是 running、带开始的时刻"
    );
    assert!(
        live.iter()
            .any(|s| matches!(s.doing, Some(Doing::Writing { .. }))),
        "写回答的时候是 writing"
    );
    assert_eq!(live.last(), Some(&status), "视图流最后和翻页一样");
}

#[test]
fn a_held_tool_keeps_it_running_on_that_step() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("read");
    let status = statuses(&stage).pop().expect("有状态");
    assert_eq!(status.state, State::Running);
    assert!(
        matches!(status.doing, Some(Doing::Tool { .. })),
        "{status:?}"
    );
    let call = stage.held_tools()[0];
    stage.release_tool(call);
    assert_eq!(paged(&stage).state, State::Idle);
}

#[test]
fn an_approval_and_a_question_are_waited_for_until_settled() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::calls("", &[("ask_user", r#"{"questions":[]}"#)]),
        Line::says("Thanks."),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    let question = Question {
        header: None,
        question: "Which one?".to_string(),
        options: vec![Choice {
            label: "A".to_string(),
            description: None,
            preview: None,
        }],
        multiple: false,
    };
    stage.tools([Play::done("written"), Play::Asks(vec![question])]);
    stage.say("write it");
    let waiting = paged(&stage);
    assert_eq!(waiting.state, State::Waiting, "{waiting:?}");
    assert_eq!(waiting.waiting.len(), 1);
    assert_eq!(waiting.waiting[0].what, Wait::Approve);
    let call = waiting.waiting[0].call;
    stage.decide(call, Decision::Once, None);
    let asking = paged(&stage);
    assert_eq!(asking.state, State::Waiting, "{asking:?}");
    assert_eq!(asking.waiting[0].what, Wait::Ask);
    assert_ne!(asking.waiting[0].call, call);
    let entry = asking.waiting[0].entry.clone();
    let question_call = asking.waiting[0].call;
    assert!(
        entry.as_str().starts_with('b'),
        "对到那一步的条目：{entry:?}"
    );
    stage.reply(
        question_call,
        vec![Response {
            picked: vec!["A".to_string()],
            text: None,
            notes: None,
            attachments: Vec::new(),
        }],
    );
    let done = paged(&stage);
    assert_eq!(done.state, State::Idle);
    assert!(done.waiting.is_empty());
}

#[test]
fn retrying_and_cooling_show_and_clear() {
    let mut stage = stage();
    stage.model([
        Line::fails(ErrorClass::Cooling, "all candidates cooling"),
        Line::says("Back."),
    ]);
    stage.say("hi");
    let live = statuses(&stage);
    let retried = live
        .iter()
        .find(|s| matches!(s.doing, Some(Doing::Retrying { .. })))
        .expect("重试时是 retrying");
    assert!(retried.cooling_until.is_some(), "都在冷却：{retried:?}");
    let Some(Doing::Retrying { at, class, .. }) = &retried.doing else {
        unreachable!("上面挑的就是 retrying");
    };
    assert_eq!(*class, ErrorClass::Cooling);
    let wait = stage
        .transients()
        .iter()
        .find_map(|t| match &t.body {
            TransientBody::Status(status) => Some((t.at, status.retry.wait_ms)),
            _ => None,
        })
        .expect("有重试的瞬时事件");
    assert_eq!(
        at.unix_millis(),
        wait.0.unix_millis() + i64::try_from(wait.1).expect("不大"),
        "再试的时刻是那一刻加上要等的"
    );
    let last = live.last().expect("有状态");
    assert_eq!(last.cooling_until, None, "说成了就不再冷却");
    assert_eq!(last.doing, None);
}

#[test]
fn jobs_are_listed_from_start_to_end() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Built."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    let running = paged(&stage);
    assert_eq!(running.jobs.len(), 1);
    let row = &running.jobs[0];
    assert_eq!(row.state, JobState::Running);
    assert_eq!(row.title, "Build");
    assert_eq!(row.command.as_deref(), Some("cargo build"));
    assert_eq!(row.ended, None);
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let done = paged(&stage);
    let row = &done.jobs[0];
    assert_eq!(row.state, JobState::Done);
    assert_eq!(row.exit_code, Some(0));
    assert!(row.ended.is_some());
}

#[test]
fn a_stopped_job_says_why() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"sleep 9","description":"Wait","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Stopped."),
    ]);
    stage.tools([Play::starts_command(1, "Wait")]);
    stage.say("wait");
    stage.job_ends(1, JobReason::Undone, By::Kernel, None);
    let row = &paged(&stage).jobs[0];
    assert_eq!(row.state, JobState::Stopped);
    assert_eq!(row.why, Some("undone"));
}

/// 到了上限停掉的（施工 7-12 下）：停了，说是哪一种上限。
#[test]
fn a_job_stopped_at_its_limit_says_which() {
    for (reason, why) in [
        (JobReason::TimedOut, "timed_out"),
        (JobReason::TooLarge, "too_large"),
    ] {
        let mut stage = stage();
        stage.model([
            Line::calls(
                "",
                &[(
                    "shell",
                    r#"{"command":"sleep 9","description":"Wait","run_in_background":true}"#,
                )],
            ),
            Line::says("Started."),
            Line::says("Stopped."),
        ]);
        stage.tools([Play::starts_command(1, "Wait")]);
        stage.say("wait");
        stage.job_ends(1, reason, By::Kernel, None);
        let row = &paged(&stage).jobs[0];
        assert_eq!(row.state, JobState::Stopped);
        assert_eq!(row.why, Some(why));
    }
}

#[test]
fn a_decided_approval_stops_waiting_while_the_tool_runs() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::says("Done."),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    stage.tools([Play::done("written").held()]);
    stage.say("write it");
    let call = paged(&stage).waiting[0].call;
    stage.decide(call, Decision::Once, None);
    let running = paged(&stage);
    assert_eq!(running.state, State::Running, "定了就不等人：{running:?}");
    assert!(running.waiting.is_empty());
}

#[test]
fn the_context_counts_all_three_inputs_and_the_output() {
    let mut stage = stage();
    stage.model([Line {
        usage: Some(Usage {
            uncached: 100,
            cache_read: 200,
            cache_write: 30,
            output: 5,
            reasoning: None,
        }),
        ..Line::says("Hi.")
    }]);
    stage.say("hello");
    // 输出也算（施工 9-8 十四补）：她回的下一次请求要带上。
    assert_eq!(paged(&stage).used, Some(335));
}

#[test]
fn only_the_latest_finished_jobs_are_kept() {
    let mut stage = stage();
    let total = miyu_view::FINISHED_KEPT as u64 + 2;
    let calls: Vec<(String, String)> = (1..=total)
        .map(|n| {
            (
                "shell".to_string(),
                format!(
                    r#"{{"command":"echo {n}","description":"Job {n}","run_in_background":true}}"#
                ),
            )
        })
        .collect();
    let calls: Vec<(&str, &str)> = calls
        .iter()
        .map(|(name, args)| (name.as_str(), args.as_str()))
        .collect();
    stage.model([Line::calls("", &calls), Line::says("Started.")]);
    stage.tools((1..=total).map(|n| Play::starts_command(n, &format!("Job {n}"))));
    stage.say("run them");
    for n in 1..=total {
        stage.model([Line::says("Noted.")]);
        stage.job_ends(n, JobReason::Exited, By::Kernel, None);
    }
    let jobs = paged(&stage).jobs;
    assert_eq!(jobs.len(), miyu_view::FINISHED_KEPT);
    assert_eq!(jobs[0].title, "Job 3", "最早做完的两个拿掉了");
}

/// 整理会话的主请求（施工 D-1 下补）：`purpose: memory` 的照主请求算上下文用了多少。
#[test]
fn a_dream_sessions_requests_count_toward_the_context() {
    use miyu_kernel::event::{Body, Purpose};
    let mut stage = stage();
    stage.model([Line::says("Done.").reports(2_048)]);
    stage.say("tidy up");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    for event in stage.log() {
        let mut event = event.clone();
        if let Body::ModelCalled(called) = &mut event.body {
            called.purpose = Some(Purpose::Memory);
        }
        drop(projector.event(&event));
    }
    assert_eq!(projector.status().used, Some(2_048));
}

/// 等人不算用时（施工 9-8 十四补，项目主人 2026-10-23 定）：等确认两分钟、等回答三分钟，状态记着等完的和正在等的，这一步、
/// 这一段、这一轮的用时都刨掉。
#[test]
fn time_spent_waiting_for_a_person_is_not_counted() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("write", r#"{"file_path":"a","content":"x"}"#)]),
        Line::calls("", &[("ask_user", r#"{"questions":[]}"#)]),
        Line::says("Thanks."),
    ]);
    stage.guards([Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Write,
        rule: None,
        detail: None,
    }]);
    let question = Question {
        header: None,
        question: "Which one?".to_string(),
        options: Vec::new(),
        multiple: false,
    };
    stage.tools([Play::done("written"), Play::Asks(vec![question])]);
    stage.say("write it");
    let first = paged(&stage);
    assert_eq!(first.waited_ms, 0);
    assert!(first.waiting_since.is_some(), "在等确认：{first:?}");
    stage.advance(2);
    let call = first.waiting[0].call;
    stage.decide(call, Decision::Once, None);
    let asking = paged(&stage);
    // 测试台的时钟每条事件走一秒：等完的那一段是拨的两分钟加上几秒。
    assert!(
        (120_000..130_000).contains(&asking.waited_ms),
        "等完的那一段：{asking:?}"
    );
    assert!(asking.waiting_since.is_some(), "又在等回答");
    stage.advance(3);
    let question_call = asking.waiting[0].call;
    stage.reply(
        question_call,
        vec![Response {
            picked: Vec::new(),
            text: Some("A".to_string()),
            notes: None,
            attachments: Vec::new(),
        }],
    );
    let done = paged(&stage);
    assert_eq!(
        (done.waited_ms, done.waiting_since),
        (0, None),
        "一轮结束不写"
    );
    let entries = same(&stage);
    for kind in ["tool", "group", "end"] {
        for entry in of_kind(&entries, kind) {
            let took = json(entry)["took_ms"].as_u64().expect("有用时");
            assert!(took < 60_000, "{kind} 刨掉了等人的五分钟：{entry:?}");
        }
    }
}

/// 同时等两件确认只算一段（施工 9-8 十四补）：两件都定了以后，命令再跑四分钟，这四分钟照样算进这一轮。
#[test]
fn waiting_for_two_things_at_once_counts_once() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[
                ("read", r#"{"file_path":"a"}"#),
                ("read", r#"{"file_path":"b"}"#),
            ],
        ),
        Line::says("ok"),
    ]);
    let ask = || Verdict::Ask {
        module: ModuleId::parse("permissions").expect("合写法"),
        access: Access::Read,
        rule: None,
        detail: None,
    };
    stage.guards([ask(), ask()]);
    stage.tools([Play::done("A").held(), Play::done("B")]);
    stage.say("read both");
    let both = paged(&stage);
    assert_eq!(both.waiting.len(), 2, "两件同时等：{both:?}");
    stage.advance(2);
    stage.decide(both.waiting[0].call, Decision::Once, None);
    stage.advance(1);
    stage.decide(both.waiting[1].call, Decision::Once, None);
    let settled = paged(&stage);
    assert_eq!(settled.waiting_since, None, "都定了：{settled:?}");
    assert!(
        (180_000..190_000).contains(&settled.waited_ms),
        "{settled:?}"
    );
    stage.advance(4);
    for call in stage.held_tools() {
        stage.release_tool(call);
    }
    let entries = same(&stage);
    let end = json(of_kind(&entries, "end")[0]);
    let took = end["took_ms"].as_u64().expect("有用时");
    assert!(
        (240_000..260_000).contains(&took),
        "跑的四分钟算、等的三分钟不算：{end}"
    );
}
