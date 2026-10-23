use miyu_kernel::event::{
    ApprovalDecided, ApprovalRequested, Body, Decision, NestedStatus, ToolNested,
};
use miyu_kernel::id::{CallId, NestedId};
use miyu_kernel::tool::Access;

use super::Asking;

fn shell() -> CallId {
    CallId::parse("call_6_1").unwrap()
}

fn requested(n: u32) -> Body {
    Body::ApprovalRequested(ApprovalRequested {
        call_id: shell(),
        access: Access::Network,
        rule: None,
        detail: None,
        nested: Some(n),
    })
}

fn decided(n: Option<u32>) -> Body {
    Body::ApprovalDecided(ApprovalDecided {
        call_id: shell(),
        decision: Decision::Once,
        reason: None,
        attachments: Vec::new(),
        nested: n,
    })
}

fn settled(n: u32) -> Body {
    Body::ToolNested(ToolNested {
        id: NestedId::parse(&format!("call_6_1#{n}")).unwrap(),
        name: "fetch".to_string(),
        args: "{}".to_string(),
        status: NestedStatus::Ok,
        duration_ms: None,
        result: None,
        bytes: None,
        reason: None,
    })
}

#[test]
fn a_question_starts_and_its_answer_ends_the_wait() {
    let mut asking = Asking::default();
    assert_eq!(asking.note(&requested(2)), Some((shell(), true)));
    assert_eq!(asking.note(&decided(Some(2))), Some((shell(), false)));
    assert_eq!(asking.note(&settled(2)), None, "等完了的不再说");
}

/// 同一个父级下面早先派出去的那一件跑完了，不算等完。
#[test]
fn another_call_under_the_same_shell_does_not_end_the_wait() {
    let mut asking = Asking::default();
    asking.note(&requested(2));
    assert_eq!(asking.note(&settled(1)), None);
    assert_eq!(
        asking.note(&settled(2)),
        Some((shell(), false)),
        "问的那一件了结了"
    );
}

#[test]
fn the_shell_ending_ends_the_wait_and_direct_calls_are_not_counted() {
    let mut asking = Asking::default();
    assert_eq!(asking.note(&decided(None)), None, "直接调用的决定不算");
    asking.note(&requested(1));
    let ended = Body::ToolResult(miyu_kernel::event::ToolResult {
        call_id: shell(),
        status: miyu_kernel::event::ToolStatus::Ok,
        blocks: Vec::new(),
        duration_ms: None,
        human: None,
        effects: Vec::new(),
        end_turn: false,
        sent: Vec::new(),
        display_name: std::collections::BTreeMap::new(),
        detail: None,
    });
    assert_eq!(asking.note(&ended), Some((shell(), false)));
}
