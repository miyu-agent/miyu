//! 合并转发里的一条怎么读（施工 O-34）：NapCat 回的完整消息、发合并转发那种 `node` 写法都认；名字先群名片、再昵称；号先
//! `sender.user_id`；段照 `message`，没有的照 `content`，字符串照 CQ 码只读字。

use serde_json::json;

use super::*;
use crate::onebot::Piece;

#[test]
fn a_napcat_message_node_is_read() {
    let read = node(&json!({
        "message_id": 1, "user_id": "20017", "time": 1,
        "sender": {"user_id": 20017, "nickname": "阿杰", "card": "  "},
        "message": [{"type": "text", "data": {"text": "嗯"}}, {"type": "image", "data": {"file": "a.jpg"}}],
    }));
    assert_eq!(read.user, Some(20017));
    assert_eq!(read.name.as_deref(), Some("阿杰"), "群名片空白的照昵称");
    assert_eq!(read.segments.pieces, [Piece::Text("嗯".to_string())]);
    assert_eq!(read.segments.media.len(), 1);
}

#[test]
fn a_send_style_node_and_a_cq_string_are_read() {
    let read = node(&json!({"type": "node", "data": {
        "user_id": "30003", "nickname": "老王", "content": "[CQ:face,id=1]你好",
    }}));
    assert_eq!(read.user, Some(30003));
    assert_eq!(read.name.as_deref(), Some("老王"));
    assert_eq!(read.segments.pieces, [Piece::Text("你好".to_string())]);
    let nameless = node(&json!({"message": []}));
    assert_eq!((nameless.user, nameless.name), (None, None));
}

#[test]
fn the_action_and_the_reply() {
    assert_eq!(
        get_forward_msg("7495"),
        ("get_forward_msg", json!({"id": "7495"}))
    );
    assert_eq!(
        forward_messages(&json!({"messages": [{"a": 1}]})),
        Some(vec![json!({"a": 1})])
    );
    assert_eq!(forward_messages(&json!({"messages": "x"})), None);
    assert_eq!(forward_messages(&json!(null)), None);
}
