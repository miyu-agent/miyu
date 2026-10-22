//! 拒绝时附图（施工 Q-A，2026-10-22 项目主人：「回答的时候用户需要图片辅助表达」）：附的图记在决定里、接在理由后面进结果；
//! 允许的带了附件拒绝，什么都不记。

use super::*;
use crate::block::Image;
use crate::id::{ContentHash, MediaType};

fn picture() -> Block {
    Block::Image(Image {
        blob: ContentHash::of(b"png"),
        name: None,
        media_type: MediaType::parse("image/png").unwrap(),
        width: 10,
        height: 10,
        path: None,
    })
}

fn with_picture(n: u64, decision: Decision) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Answer {
            call_id: call(6, 1),
            answer: Answer::Approval {
                decision,
                reason: Some("看图".to_string()),
                attachments: vec![picture()],
            },
        },
    })
}

#[test]
fn a_denial_carries_its_pictures_after_the_reason() {
    let mut session = waiting_for_you(true);
    let actions = session.handle(with_picture(2, Decision::Deny));
    let events = appended_events(&actions);
    assert_eq!(decision_of(&events[0]).attachments, vec![picture()]);
    let Body::ToolResult(result) = &events[1].body else {
        panic!("应该是 tool.result：{:?}", events[1]);
    };
    assert_eq!(result.status, ToolStatus::Denied);
    assert!(matches!(&result.blocks[0], Block::Text(text) if text.text == "denied: 看图"));
    assert_eq!(result.blocks[1..], [picture()], "图接在理由后面");
}

#[test]
fn allowing_with_pictures_is_refused() {
    let mut session = waiting_for_you(true);
    assert_eq!(
        session.handle(with_picture(2, Decision::Once)),
        rejected_with(Reason::UnexpectedReason, 2)
    );
}
