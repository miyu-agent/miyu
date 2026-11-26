//! 驱动在一次请求进行中问人（施工 ACP-8，`driver_ask.rs`）：问到了记请求、画在哪一块；人允许、拒绝落了盘才回；问不了的当场回；
//! 到时没人确认；这一次有了结局（说完了、打断）还在等的了结；问的不是这一次、不是供应商侧工具的。

use super::approval::answer;
use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{ApprovalRequested, Decision, DriverBlock};
use crate::id::ModuleId;
use crate::origin::Module;
use crate::raw::RawJson;

/// 请求 5 发出去了，第 0 块是一件供应商侧工具 `Bash`（开了、还没收全）。
fn hosting(session: &mut Session) {
    session.handle(sent(5));
    let start = Delta::Start {
        index: 0,
        kind: Kind::HostedTool {
            name: "Bash".to_string(),
        },
    };
    session.handle(delta(5, 41, start));
}

/// 包 `acp` 的驱动在 07:00:42 问请求 `seen` 的第 `index` 块，要 `access`，编号 `token`。
fn asked(seen: u64, index: u32, access: Access, token: u64) -> Input {
    Input::DriverAsked(Box::new(DriverAsked {
        at: at(42),
        seen: seq(seen),
        token,
        module: ModuleId::parse("acp").unwrap(),
        access,
        index,
        detail: serde_json::from_str::<RawJson>(r#"{"command":"ls","driver":"acp/claude"}"#)
            .unwrap(),
    }))
}

/// 动作里回驱动的那几句：编号、怎么答。
fn answered(actions: &[Action]) -> Vec<(u64, DriverAnswer)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::AnswerDriver { token, answer } => Some((*token, answer.clone())),
            _ => None,
        })
        .collect()
}

fn request_of(event: &Event) -> &ApprovalRequested {
    match &event.body {
        Body::ApprovalRequested(request) => request,
        body => panic!("应该是 tool.approval_requested：{body:?}"),
    }
}

/// 一个开了回合、请求 5 在第 0 块跑供应商侧工具的会话，驱动问了一次（编号 7），请求是 6 号、落了盘。
fn waiting() -> Session {
    let mut session = asking();
    hosting(&mut session);
    let actions = session.handle(asked(5, 0, Access::Execute, 7));
    assert_eq!(appended(&actions), seqs(&[6]), "{actions:?}");
    session.handle(stored(6));
    session
}

#[test]
fn a_driver_asking_records_a_request_drawn_on_its_hosted_block() {
    let mut session = asking();
    hosting(&mut session);
    let actions = session.handle(asked(5, 0, Access::Execute, 7));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6]));
    assert_eq!(
        events[0].by,
        By::Module(Module {
            id: ModuleId::parse("acp").unwrap()
        })
    );
    let request = request_of(&events[0]);
    assert_eq!(
        request.call_id,
        call(6, 1),
        "内核另给的编号：这一条的序号、第 1 个"
    );
    assert_eq!(
        request.driver,
        Some(DriverBlock {
            seen: seq(5),
            index: 0
        })
    );
    assert_eq!(request.access, Access::Execute);
    assert!(request.rule.is_none(), "没有规则：只能允许这一次或者拒绝");
    assert_eq!(
        request.detail.as_ref().map(RawJson::get),
        Some(r#"{"command":"ls","driver":"acp/claude"}"#)
    );
    assert!(answered(&actions).is_empty(), "等人：还没回驱动");
}

#[test]
fn the_person_allowing_answers_the_driver_once_the_decision_is_stored() {
    let mut session = waiting();
    let actions = session.handle(answer(8, call(6, 1), Decision::Once, None));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[7]));
    assert_eq!(events[0].by, person());
    assert!(answered(&actions).is_empty(), "决定还没落盘");
    let actions = session.handle(stored(7));
    assert_eq!(answered(&actions), [(7, DriverAnswer::Allowed)]);
    assert!(
        actions.contains(&accepted_reply(8, &[7])),
        "命令照常回：{actions:?}"
    );
    assert_eq!(
        session.handle(answer(9, call(6, 1), Decision::Deny, None)),
        vec![rejected(id(9), Reason::NotAsking)],
        "答过了"
    );
}

#[test]
fn the_person_denying_passes_the_reason_and_remembering_is_refused() {
    let mut session = waiting();
    assert_eq!(
        session.handle(answer(8, call(6, 1), Decision::Session, None)),
        vec![rejected(id(8), Reason::NoRule)],
        "没有规则，不能记住"
    );
    session.handle(answer(9, call(6, 1), Decision::Deny, Some("不要删")));
    let actions = session.handle(stored(7));
    assert_eq!(
        answered(&actions),
        [(
            7,
            DriverAnswer::Denied {
                reason: Some("不要删".to_string())
            }
        )]
    );
}

#[test]
fn where_no_one_can_approve_the_driver_is_refused_on_the_spot() {
    let mut policy = policy();
    policy.attended = false;
    let mut session = session_with(policy);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    hosting(&mut session);
    let actions = session.handle(asked(5, 0, Access::Execute, 7));
    assert!(appended(&actions).is_empty(), "不记请求：{actions:?}");
    assert_eq!(
        answered(&actions),
        [(7, DriverAnswer::Auto(AutoDeny::Unattended))]
    );
}

#[test]
fn read_only_refuses_running_and_writing_but_not_reading() {
    use super::permission::read_only;
    let mut session = asking();
    session.handle(read_only(2, true));
    hosting(&mut session);
    let actions = session.handle(asked(5, 0, Access::Execute, 7));
    assert_eq!(
        answered(&actions),
        [(7, DriverAnswer::Auto(AutoDeny::ReadOnly))],
        "对方跑的命令不在沙盒里"
    );
    let actions = session.handle(asked(5, 0, Access::Read, 8));
    assert!(answered(&actions).is_empty(), "读的照问：{actions:?}");
}

#[test]
fn asking_about_another_request_or_a_block_that_is_no_hosted_tool_is_refused() {
    let mut session = asking();
    hosting(&mut session);
    let actions = session.handle(asked(4, 0, Access::Execute, 7));
    assert_eq!(answered(&actions), [(7, DriverAnswer::Cancelled)]);
    let actions = session.handle(asked(5, 1, Access::Execute, 8));
    assert_eq!(
        answered(&actions),
        [(8, DriverAnswer::NotHosted)],
        "还没开始的块"
    );
    for delta_ in words(1, "hi") {
        session.handle(delta(5, 43, delta_));
    }
    let actions = session.handle(asked(5, 1, Access::Execute, 9));
    assert_eq!(answered(&actions), [(9, DriverAnswer::NotHosted)], "正文块");
    assert!(appended(&actions).is_empty());
}

#[test]
fn interrupting_settles_what_is_still_waiting_and_tells_the_driver_it_is_gone() {
    let mut session = waiting();
    let actions = session.handle(interrupt(8));
    let events = appended_events(&actions);
    let decided: Vec<&Event> = events
        .iter()
        .filter(|event| matches!(event.body, Body::ApprovalDecided(_)))
        .collect();
    assert_eq!(decided.len(), 1, "{events:?}");
    assert_eq!(decided[0].by, By::Kernel);
    let Body::ApprovalDecided(body) = &decided[0].body else {
        unreachable!()
    };
    assert_eq!(
        (body.call_id, &body.decision),
        (call(6, 1), &Decision::Deny)
    );
    let reply = events
        .iter()
        .position(|event| matches!(event.body, Body::MessageAssistant(_)));
    let decided_at = events
        .iter()
        .position(|event| matches!(event.body, Body::ApprovalDecided(_)));
    assert!(decided_at < reply, "先了结，再写回复：{events:?}");
    assert_eq!(answered(&actions), [(7, DriverAnswer::Cancelled)], "当场回");
    assert_eq!(
        session.handle(answer(9, call(6, 1), Decision::Once, None)),
        vec![rejected(id(9), Reason::NotAsking)],
        "了结了"
    );
}

#[test]
fn the_request_ending_while_asking_settles_it_inside_the_turn() {
    let mut session = waiting();
    let actions = session.handle(ended(5));
    let events = appended_events(&actions);
    let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
    assert_eq!(kinds.first(), Some(&"tool.approval_decided"), "{kinds:?}");
    assert_eq!(kinds.last(), Some(&"turn.ended"), "回合照常结束");
    assert_eq!(answered(&actions), [(7, DriverAnswer::Cancelled)]);
}

#[test]
fn no_one_deciding_in_time_denies_for_the_person() {
    let mut session = waiting();
    let expired = Input::ApprovalExpired {
        at: at(50),
        call_id: call(6, 1),
        nested: None,
        text: "timed out".to_string(),
        human: None,
    };
    let actions = session.handle(expired);
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[7]), "只记决定：没有工具结果");
    assert_eq!(events[0].by, By::Kernel);
    assert!(answered(&actions).is_empty(), "落了盘再回");
    let actions = session.handle(stored(7));
    assert_eq!(
        answered(&actions),
        [(7, DriverAnswer::Auto(AutoDeny::Expired))]
    );
}
