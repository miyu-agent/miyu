//! 那一句接在哪（施工 RT-1）：最后是 user 的并进去，别的另起一条 user；没有、空的、跳过的照原样。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::CallId;
use miyu_kernel::request::{Message, Request};

use super::tailed;

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn request(messages: Vec<Message>, tail: Option<&str>) -> Request {
    Request {
        tools: Vec::new(),
        system: String::new(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
        tail: tail.map(str::to_string),
    }
}

#[test]
fn it_joins_the_last_user_message_or_starts_one() {
    let user = Message::User {
        blocks: vec![text("在吗")],
    };
    let original = request(vec![user.clone()], Some("提示"));
    let (out, added) = tailed(&original, false);
    assert!(added);
    assert_eq!(
        out.messages,
        vec![Message::User {
            blocks: vec![text("在吗"), text("提示")]
        }]
    );
    assert_eq!(out.tail, None, "接上了就不再留在那一格里");
    let tool = Message::Tool {
        call_id: CallId::parse("call_1_1").unwrap(),
        error: false,
        blocks: vec![text("结果")],
    };
    let original = request(vec![user.clone(), tool.clone()], Some("提示"));
    let (out, added) = tailed(&original, false);
    assert!(added);
    assert_eq!(
        out.messages,
        vec![
            user.clone(),
            tool,
            Message::User {
                blocks: vec![text("提示")]
            }
        ]
    );
}

#[test]
fn nothing_empty_or_skipped_borrows_the_request_as_it_is() {
    let user = Message::User {
        blocks: vec![text("在吗")],
    };
    for (tail, skip) in [(None, false), (Some(""), false), (Some("提示"), true)] {
        let original = request(vec![user.clone()], tail);
        let (out, added) = tailed(&original, skip);
        assert!(!added, "{tail:?} {skip}");
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert_eq!(out.messages, original.messages);
    }
}
