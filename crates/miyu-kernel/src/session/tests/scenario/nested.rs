//! 场景：脚本里的 `miyu call`（施工 M-1，`docs/blueprint/kernel/session.md`「嵌套调用」）。前台 `shell` 下面几次调用照
//! 队派、先记完再记父调用的结果；父级先结束的收尾；链的几种结论；查不过的不记；两道上限；后台任务下面的在回合外记、任务
//! 报告之前收齐；打断、崩了。

use super::*;
use crate::event::{JobReason, NestedStatus, TransientBody};
use crate::id::{CallId, NestedId, TurnId};
use crate::session::{NestedAnswer, Verdict};

/// 那次 `shell` 的调用编号：第一轮的回复是 6 号，第一个调用。
pub(super) const SHELL: &str = "call_6_1";

pub(super) fn nested(id: &str) -> NestedId {
    NestedId::parse(id).unwrap()
}

/// 一个替身，嵌套调用的两道上限照给的。
fn stage_with(per_parent: usize, per_session: usize) -> Stage {
    let make = move || {
        let mut made = policy();
        made.nested = crate::session::NestedLimits {
            per_parent,
            per_session,
        };
        made
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// 她跑一个脚本：第一轮调 `shell`，`shell` 停在跑着；结果回来以后说「好了」。
pub(super) fn running_shell(stage: &mut Stage) {
    stage.model([
        Line::calls(
            "我写个脚本。",
            &[("shell", r#"{"command":"python3 x.py"}"#)],
        ),
        Line::says("好了。"),
    ]);
    stage.tools([Play::done("done").held()]);
    stage.say("数一下");
}

/// 嵌套调用了结的几条：编号和结局，照日志的先后。
pub(super) fn settled(stage: &Stage) -> Vec<String> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolNested(n) => Some(format!("{} {}", n.id, n.status.as_str())),
            _ => None,
        })
        .collect()
}

/// 答脚本那头的：编号和结局。
pub(super) fn answered(stage: &Stage) -> Vec<String> {
    stage
        .nested_answers()
        .iter()
        .map(|(id, answer)| match answer {
            NestedAnswer::Settled { status, .. } => format!("{id} {}", status.as_str()),
            NestedAnswer::Refused(refusal) => format!("{id} refused {refusal:?}"),
        })
        .collect()
}

#[test]
fn nested_calls_settle_before_the_shell_and_run_in_order() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", r#"{"path":"a"}"#);
    stage.nested_call("call_6_1#2", "read", r#"{"path":"b"}"#);
    stage.nested_call("call_6_1#3", "shell", r#"{"command":"ls"}"#);
    // 读的两个一起跑，执行的那个等前面的了结；父 `shell` 独占着这一步也照样派。
    assert_eq!(
        stage.nested_running(),
        [nested("call_6_1#1"), nested("call_6_1#2")]
    );
    // 头重连时拿到的在跑的：照编号。
    assert_eq!(
        stage.nested_in_flight(),
        ["call_6_1#1 read", "call_6_1#2 read"]
    );
    stage.nested_done("call_6_1#2", false);
    assert_eq!(stage.nested_running(), [nested("call_6_1#1")]);
    stage.nested_done("call_6_1#1", true);
    assert_eq!(stage.nested_running(), [nested("call_6_1#3")]);
    stage.nested_done("call_6_1#3", false);
    stage.release_tool(CallId::parse(SHELL).unwrap());
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 tool.nested kernel t3",
            "9 tool.nested kernel t3",
            "10 tool.nested kernel t3",
            "11 tool.result:ok tool call_6_1 t3",
            "12 message.assistant model t3",
            "13 model.called:ok kernel t3",
            "14 turn.ended:completed kernel t3",
        ])
    );
    assert_eq!(
        settled(&stage),
        ["call_6_1#2 ok", "call_6_1#1 error", "call_6_1#3 ok"]
    );
    // 落了盘才答；结果的 blob、用时照交的。
    assert_eq!(
        answered(&stage),
        ["call_6_1#2 ok", "call_6_1#1 error", "call_6_1#3 ok"]
    );
    let Body::ToolNested(first) = &stage.log()[7].body else {
        panic!("8 号应该是嵌套调用");
    };
    assert_eq!((first.duration_ms, first.bytes), (Some(12), Some(7)));
    assert_eq!(first.args, r#"{"path":"b"}"#);
    // 推给头的开始了：三个，都带这一轮。
    let started: Vec<_> = stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::NestedStarted(started) => Some((started.id.to_string(), transient.turn)),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 3);
    assert!(
        started
            .iter()
            .all(|(_, turn)| *turn == Some(TurnId::new(seq(3))))
    );
}

#[test]
fn a_shell_that_ends_first_cancels_what_hangs_under_it() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "shell", "{}");
    stage.release_tool(CallId::parse(SHELL).unwrap());
    assert_eq!(
        settled(&stage),
        ["call_6_1#1 cancelled", "call_6_1#2 cancelled"]
    );
    let lines = story(&stage);
    assert_eq!(lines[7], "8 tool.nested kernel t3");
    assert_eq!(lines[8], "9 tool.nested kernel t3");
    assert_eq!(lines[9], "10 tool.result:ok tool call_6_1 t3");
    assert_eq!(
        stage.nested_cancelled(),
        [nested("call_6_1#1")],
        "在跑的掐掉"
    );
    // 迟到的结果不理；父调用没了以后再来的答 gone，什么都不记。
    let before = stage.log().len();
    stage.nested_done_late("call_6_1#1");
    stage.nested_call("call_6_1#3", "read", "{}");
    assert_eq!(stage.log().len(), before);
    assert_eq!(answered(&stage).last().unwrap(), "call_6_1#3 refused Gone");
}

/// 没人能确认的会话（施工 M-7 上起：有人能确认的前台调用要问人时停下来问，`nested_asking.rs`）：链拒的、要问人的当场了结。
#[test]
fn the_chain_and_the_checks_settle_or_refuse_on_the_spot() {
    let unattended = || {
        let mut made = policy();
        made.attended = false;
        made
    };
    let mut stage = Stage::new(unattended, environment("~/src/miyu"), at(0));
    running_shell(&mut stage);
    stage.nested_guards([
        Verdict::Deny {
            module: crate::id::ModuleId::parse("permissions").unwrap(),
            text: "outside the workspace".to_string(),
            human: None,
        },
        Verdict::Ask {
            module: crate::id::ModuleId::parse("permissions").unwrap(),
            access: crate::tool::Access::Read,
            rule: None,
            detail: None,
        },
    ]);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "read", "{}");
    stage.nested_call("call_6_1#3", "write", "{}");
    stage.nested_call("call_6_1#4", "nope", "{}");
    stage.nested_call("call_6_1#5", "read", "[1]");
    // 同一个编号再来，不理。
    stage.nested_call("call_6_1#1", "read", "{}");
    assert_eq!(
        settled(&stage),
        ["call_6_1#1 denied", "call_6_1#2 needs_approval"]
    );
    assert_eq!(
        answered(&stage),
        [
            "call_6_1#1 denied",
            "call_6_1#2 needs_approval",
            "call_6_1#3 refused NotAllowed",
            "call_6_1#4 refused Unknown",
            "call_6_1#5 refused NotAnObject",
        ]
    );
    let Body::ToolNested(denied) = &stage.log()[7].body else {
        panic!("8 号应该是嵌套调用");
    };
    assert_eq!(denied.reason.as_deref(), Some("outside the workspace"));
    assert_eq!(denied.status, NestedStatus::Denied);
    assert!(stage.nested_ran().is_empty(), "一个都没跑");
}

/// `clicall` 没开的（施工 M-2）：类别准也不收，当场答「不准」，什么都不记、不交给链。
#[test]
fn a_tool_without_clicall_is_refused_on_the_spot() {
    let make = || {
        let mut made = policy();
        made.tools.get_mut("read").unwrap().clicall = false;
        made
    };
    let mut stage = Stage::new(make, environment("~/src/miyu"), at(0));
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    assert_eq!(answered(&stage), ["call_6_1#1 refused NotAllowed"]);
    assert!(settled(&stage).is_empty(), "没收下的不记");
    assert!(stage.nested_ran().is_empty());
}

#[test]
fn the_limits_queue_the_rest_without_refusing() {
    let mut stage = stage_with(2, 16);
    running_shell(&mut stage);
    for n in 1..=3 {
        stage.nested_call(&format!("call_6_1#{n}"), "read", "{}");
    }
    assert_eq!(stage.nested_guarded().len(), 2, "一个父级最多两个");
    stage.nested_done("call_6_1#1", false);
    assert_eq!(
        stage.nested_running(),
        [nested("call_6_1#2"), nested("call_6_1#3")]
    );
}

/// 第一轮派一个后台命令 `j1`，那一轮结束；`j1` 还在跑。
fn running_job(stage: &mut Stage) {
    stage.model([
        Line::calls("放后台跑。", &[("shell", r#"{"command":"./count.sh"}"#)]),
        Line::says("在跑了。"),
    ]);
    stage.tools([Play::starts_command(1, "数一下")]);
    stage.say("后台数一下");
}

/// 后台任务下面的要问人照旧不问（施工 M-7 上，项目主人 10-23 定）：没人等着答，当场 `needs_approval`。
#[test]
fn under_a_job_a_question_is_still_not_asked() {
    let mut stage = stage();
    running_job(&mut stage);
    stage.nested_guards([Verdict::Ask {
        module: crate::id::ModuleId::parse("permissions").unwrap(),
        access: crate::tool::Access::Network,
        rule: None,
        detail: None,
    }]);
    stage.nested_call("j1#1", "read", "{}");
    assert_eq!(settled(&stage), ["j1#1 needs_approval"]);
    assert!(
        !stage
            .log()
            .iter()
            .any(|event| matches!(event.body, Body::ApprovalRequested(_))),
        "不记请求"
    );
}

#[test]
fn under_a_job_they_settle_outside_any_turn_and_before_its_report() {
    let mut stage = stage_with(8, 2);
    running_job(&mut stage);
    let quiet = stage.log().len();
    stage.nested_call("j1#1", "read", "{}");
    stage.nested_done("j1#1", false);
    let lines = story(&stage);
    assert_eq!(lines.len(), quiet + 1, "不开回合：{lines:?}");
    assert_eq!(
        lines.last().unwrap(),
        &format!("{} tool.nested kernel", quiet + 1)
    );
    // 会话的上限也管后台的：两个占满了，前台的排着。
    stage.nested_call("j1#2", "read", "{}");
    stage.nested_call("j1#3", "shell", "{}");
    stage.nested_call("j1#4", "read", "{}");
    assert_eq!(stage.nested_running(), [nested("j1#2")]);
    // 任务结束：还没了结的先记已取消，再记报告。
    stage.model([Line::says("数完了。")]);
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let tail: Vec<String> = settled(&stage).into_iter().skip(1).collect();
    assert_eq!(tail, ["j1#2 cancelled", "j1#3 cancelled", "j1#4 cancelled"]);
    let reported = stage
        .log()
        .iter()
        .position(|event| matches!(event.body, Body::JobReported(_)))
        .unwrap();
    assert!(matches!(
        stage.log()[reported - 1].body,
        Body::ToolNested(_)
    ));
    assert!(stage.log()[reported - 1].turn.is_none());
    // 报告以后再来的答 gone。
    stage.nested_call("j1#5", "read", "{}");
    assert_eq!(answered(&stage).last().unwrap(), "j1#5 refused Gone");
}

#[test]
fn an_interrupt_cancels_them_before_the_shell() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.interrupt(Queued::Send);
    let lines = story(&stage);
    assert_eq!(lines[7], "8 tool.nested kernel t3");
    assert_eq!(lines[8], "9 tool.result:cancelled person t3");
    assert_eq!(settled(&stage), ["call_6_1#1 cancelled"]);
    assert_eq!(stage.nested_cancelled(), [nested("call_6_1#1")]);
}

#[test]
fn after_a_crash_nothing_is_made_up_for_them() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.crash();
    assert!(settled(&stage).is_empty(), "崩了不补");
    assert!(stage.nested_running().is_empty());
}

#[test]
fn undoing_the_turn_takes_its_nested_calls_away() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_done("call_6_1#1", false);
    stage.release_tool(CallId::parse(SHELL).unwrap());
    let before = listed_request(&stage.requests()[1].1);
    assert!(
        before.contains("tool.nested"),
        "替身的组装列出有效历史：{before}"
    );
    stage.revert(TurnId::new(seq(3)));
    stage.model([Line::says("重新来。")]);
    stage.say("再数一次");
    let after = listed_request(&stage.requests().last().unwrap().1);
    assert!(
        !after.contains("tool.nested"),
        "撤掉的那一轮的嵌套调用跟着走：{after}"
    );
}

#[test]
fn the_session_limit_holds_across_parents() {
    let mut stage = stage_with(8, 2);
    stage.model([
        Line::calls(
            "两个都放后台。",
            &[
                ("shell", r#"{"command":"a"}"#),
                ("shell", r#"{"command":"b"}"#),
            ],
        ),
        Line::says("在跑了。"),
    ]);
    stage.tools([Play::starts_command(1, "a"), Play::starts_command(2, "b")]);
    stage.say("放后台");
    stage.nested_call("j1#1", "read", "{}");
    stage.nested_call("j1#2", "read", "{}");
    stage.nested_call("j2#1", "read", "{}");
    assert_eq!(
        stage.nested_running(),
        [nested("j1#1"), nested("j1#2")],
        "整个会话最多两个，j2 的排着"
    );
    stage.nested_done("j1#1", false);
    assert_eq!(stage.nested_running(), [nested("j1#2"), nested("j2#1")]);
}

#[test]
fn the_script_hears_back_only_after_the_settling_lands() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "read", "{}");
    stage.hold_stores();
    stage.nested_done("call_6_1#1", false);
    stage.nested_done("call_6_1#2", false);
    assert_eq!(
        settled(&stage),
        ["call_6_1#1 ok", "call_6_1#2 ok"],
        "都记了"
    );
    assert!(stage.nested_answers().is_empty(), "还没落盘，不答");
    // 只落到第一条：只答第一个。
    let first = stage.log().len() as u64 - 1;
    stage.store_upto(first);
    assert_eq!(answered(&stage), ["call_6_1#1 ok"]);
    stage.release_stores();
    assert_eq!(answered(&stage), ["call_6_1#1 ok", "call_6_1#2 ok"]);
}

/// 执行器照它开通道、答 `--schema`（施工 M-3）：`clicall` 开着、类别准的才列，照名字，带参数格式；写的（`write`）开着也不列，
/// `clicall` 关的不列。
#[test]
fn the_tools_a_script_may_call_are_listed_by_name() {
    let mut made = policy();
    made.tools
        .get_mut("ask_user")
        .expect("替身的策略有 ask_user")
        .clicall = false;
    let session = session_with(made);
    let listed: Vec<String> = session
        .clicall_tools()
        .into_iter()
        .map(|(name, parameters)| {
            assert!(parameters.get().starts_with('{'));
            name
        })
        .collect();
    assert_eq!(listed, ["read", "shell"]);
}
