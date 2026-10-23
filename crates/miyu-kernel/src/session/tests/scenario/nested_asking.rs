//! 场景：脚本里调的要人确认（施工 M-7 上，`docs/blueprint/kernel/asking.md`「脚本里调的要人确认」）。前台、有人能确认的
//! 主会话里停下来问，请求记在父 `shell` 上、带序号；允许的派去跑、拒绝的了结成 `denied`；不带序号的回答答正在问的那一件；
//! 一个父级同时只问一件，第二件退回排队、前一件答了再过链，带上「本会话都允许」的规则；父级先结束的收尾。子会话里的、
//! 后台任务下面的照旧 `needs_approval`。

use super::nested::{SHELL, answered, nested, running_shell, settled};
use super::*;
use crate::event::{ApprovalRequested, Decision, NestedStatus, TransientBody};
use crate::id::{CallId, ModuleId};
use crate::raw::RawJson;
use crate::session::{Outcome, Reason, Verdict};
use crate::tool::Access;

/// 链说要联网、问人；`rule` 是提的放行规则。
fn ask(rule: Option<&str>) -> Verdict {
    Verdict::Ask {
        module: ModuleId::parse("permissions").unwrap(),
        access: Access::Network,
        rule: rule.map(|rule| serde_json::from_str::<RawJson>(rule).unwrap()),
        detail: Some(serde_json::from_str(r#"{"tool":"read"}"#).unwrap()),
    }
}

fn shell() -> CallId {
    CallId::parse(SHELL).unwrap()
}

/// 日志里的请求：调用编号、序号、谁问的。
fn requests(stage: &Stage) -> Vec<(ApprovalRequested, By)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalRequested(request) => Some((request.clone(), event.by.clone())),
            _ => None,
        })
        .collect()
}

/// 日志里的决定：序号、选项。
fn decisions(stage: &Stage) -> Vec<(Option<u32>, Decision)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ApprovalDecided(decided) => Some((decided.nested, decided.decision.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_foreground_script_waits_for_the_person_and_runs_when_allowed() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    let asked = requests(&stage);
    assert_eq!(asked.len(), 1, "记一条请求");
    let (request, by) = &asked[0];
    assert_eq!(
        (request.call_id, request.nested, &request.access),
        (shell(), Some(1), &Access::Network),
        "挂在父 shell 上、带序号"
    );
    assert_eq!(
        *by,
        By::Module(crate::origin::Module {
            id: ModuleId::parse("permissions").unwrap()
        })
    );
    assert!(stage.nested_ran().is_empty(), "等人时不派");
    assert!(answered(&stage).is_empty(), "等人时不答脚本");
    assert!(stage.nested_running().is_empty());
    let decided = stage.decide_nested(shell(), Some(1), Decision::Once, None);
    assert_eq!(decisions(&stage), [(Some(1), Decision::Once)]);
    assert!(matches!(
        stage.outcome(&decided),
        Some(Outcome::Accepted { .. })
    ));
    assert_eq!(
        stage.nested_running(),
        [nested("call_6_1#1")],
        "允许了派去跑"
    );
    assert!(
        stage.transients().iter().any(|transient| matches!(
            &transient.body,
            TransientBody::NestedStarted(started) if started.id == nested("call_6_1#1")
        )),
        "推开始"
    );
    stage.nested_done("call_6_1#1", false);
    assert_eq!(settled(&stage), ["call_6_1#1 ok"]);
    assert_eq!(answered(&stage), ["call_6_1#1 ok"]);
}

#[test]
fn a_denial_settles_it_with_the_words_of_a_direct_call() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.decide_nested(shell(), Some(1), Decision::Deny, Some("别联网"));
    assert_eq!(settled(&stage), ["call_6_1#1 denied"]);
    let reason = stage.log().iter().find_map(|event| match &event.body {
        Body::ToolNested(nested) => nested.reason.clone(),
        _ => None,
    });
    let expected = policy().tool_texts.denied(Some("别联网")).text;
    assert_eq!(reason, Some(expected), "照直接调用的那一句");
    assert!(stage.nested_ran().is_empty(), "拒了不跑");
    assert_eq!(answered(&stage), ["call_6_1#1 denied"]);
}

/// 旧的头不认 `nested`：不带序号的回答，这次调用自己没在问，就答它下面正在问的那一件（B，核心主会话 10-23 同意）。
#[test]
fn an_answer_without_the_number_reaches_the_one_asking() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    let decided = stage.decide(shell(), Decision::Once, None);
    assert!(matches!(
        stage.outcome(&decided),
        Some(Outcome::Accepted { .. })
    ));
    assert_eq!(
        decisions(&stage),
        [(Some(1), Decision::Once)],
        "决定照写上序号"
    );
    assert_eq!(stage.nested_running(), [nested("call_6_1#1")]);
}

#[test]
fn a_wrong_number_or_nothing_asking_is_not_asking() {
    let mut stage = stage();
    running_shell(&mut stage);
    let none = stage.decide(shell(), Decision::Once, None);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    let wrong = stage.decide_nested(shell(), Some(2), Decision::Once, None);
    let no_rule = stage.decide_nested(shell(), Some(1), Decision::Session, None);
    for id in [none, wrong] {
        assert_eq!(
            stage.outcome(&id),
            Some(&Outcome::Rejected {
                reason: Reason::NotAsking,
                about: Vec::new(),
            })
        );
    }
    assert_eq!(
        stage.outcome(&no_rule),
        Some(&Outcome::Rejected {
            reason: Reason::NoRule,
            about: Vec::new(),
        }),
        "没提规则的选不了本会话都允许"
    );
    assert!(decisions(&stage).is_empty(), "拒绝的什么都不记");
}

/// 一个父级同时只问一件（A，核心主会话 10-23 同意）：在问的时候同一个父级后面来的排着、不交给链；前一件选了「本会话都允许」
/// 以后，排着的再过链时带着那条规则，链放行就直接跑，不再问。两件同时在过链、后到结论的退回排队见下一条。
#[test]
fn one_question_at_a_time_and_the_next_comes_with_the_rule() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_guards([ask(Some(r#"{"host":"example.com"}"#))]);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "read", "{}");
    assert_eq!(requests(&stage).len(), 1, "同一个父级只问一件");
    let guarded = stage.nested_guarded().len();
    stage.nested_call("call_6_1#3", "read", "{}");
    assert_eq!(
        stage.nested_guarded().len(),
        guarded,
        "在问人的父级不再交给链"
    );
    stage.decide_nested(shell(), Some(1), Decision::Session, None);
    let guards = stage.nested_grants();
    let again: Vec<&Vec<RawJson>> = guards
        .iter()
        .filter(|(id, _)| *id == nested("call_6_1#2"))
        .map(|(_, grants)| grants)
        .collect();
    assert_eq!(again.len(), 1, "答了以后排着的才过链");
    let grants = again[0];
    assert_eq!(grants.len(), 1, "带上放行过的规则");
    assert_eq!(requests(&stage).len(), 1, "链放行了就不再问");
    assert!(stage.nested_running().contains(&nested("call_6_1#2")));
}

/// 两件同时在过链、都要问人：先到结论的问，后到的退回排队、不记请求；前一件答了，它再过链，这回链放行就直接跑。
#[test]
fn a_second_question_under_the_same_shell_goes_back_to_the_queue() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.hold_nested_guards();
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "read", "{}");
    assert_eq!(
        stage.nested_guarded(),
        [nested("call_6_1#1"), nested("call_6_1#2")],
        "两件读的一起过链"
    );
    stage.nested_verdict("call_6_1#2", ask(Some("{}")));
    stage.nested_verdict("call_6_1#1", ask(Some("{}")));
    let asked = requests(&stage);
    assert_eq!(asked.len(), 1, "只问一件");
    assert_eq!(asked[0].0.nested, Some(2), "先到结论的那件问");
    assert_eq!(stage.nested_guarded().len(), 2, "退回的不马上再过链");
    stage.decide_nested(shell(), None, Decision::Session, None);
    assert_eq!(
        stage.nested_guarded().last(),
        Some(&nested("call_6_1#1")),
        "答了以后退回的再过链"
    );
    stage.nested_verdict("call_6_1#1", Verdict::Allow);
    assert_eq!(requests(&stage).len(), 1, "链放行了就不再问");
    assert_eq!(
        stage.nested_running(),
        [nested("call_6_1#2"), nested("call_6_1#1")]
    );
}

/// 在问的占名额：一个父级上限 1 时，第二件排着、不交给链。
#[test]
fn the_one_asking_takes_a_place() {
    let make = || {
        let mut made = policy();
        made.nested = crate::session::NestedLimits {
            per_parent: 1,
            per_session: 16,
        };
        made
    };
    let mut stage = Stage::new(make, environment("~/src/miyu"), at(0));
    running_shell(&mut stage);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.nested_call("call_6_1#2", "read", "{}");
    assert_eq!(stage.nested_guarded(), [nested("call_6_1#1")]);
}

/// 等人时父 `shell` 先结束：在问的记 `cancelled`、排在父级结果前面，之后的回答 `not_asking`。
#[test]
fn a_shell_that_ends_while_asking_cancels_it() {
    let mut stage = stage();
    running_shell(&mut stage);
    stage.nested_guards([ask(None)]);
    stage.nested_call("call_6_1#1", "read", "{}");
    stage.release_tool(shell());
    assert_eq!(settled(&stage), ["call_6_1#1 cancelled"]);
    let lines: Vec<String> = stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolNested(_) => Some("nested".to_string()),
            Body::ToolResult(_) => Some("result".to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(lines, ["nested", "result"]);
    let late = stage.decide(shell(), Decision::Once, None);
    assert_eq!(
        stage.outcome(&late),
        Some(&Outcome::Rejected {
            reason: Reason::NotAsking,
            about: Vec::new(),
        })
    );
    assert!(stage.nested_ran().is_empty());
}

/// 子会话里的不问（项目主人 10-23 定）：它的 `attended` 照父会话的，内核照 `session.created` 的 `parent` 认。
#[test]
fn a_child_session_does_not_ask() {
    let mut stage = Stage::child(
        policy,
        environment("~/src/miyu"),
        at(0),
        super::upward::PARENT,
    );
    stage.model([
        Line::calls(
            "我写个脚本。",
            &[("shell", r#"{"command":"python3 x.py"}"#)],
        ),
        Line::says("好了。"),
    ]);
    stage.tools([Play::done("done").held()]);
    stage.say("数一下");
    let call = stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::MessageAssistant(_) => CallId::new(event.seq, 1),
            _ => None,
        })
        .unwrap();
    stage.nested_guards([ask(None)]);
    stage.nested_call(&format!("{call}#1"), "read", "{}");
    assert!(requests(&stage).is_empty());
    assert_eq!(settled(&stage), [format!("{call}#1 needs_approval")]);
    let Some(Body::ToolNested(settled)) = stage.log().last().map(|event| &event.body) else {
        panic!("最后一条应该是嵌套调用");
    };
    assert_eq!(settled.status, NestedStatus::NeedsApproval);
}
