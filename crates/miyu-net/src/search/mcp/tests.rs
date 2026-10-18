//! 回应的读法：一段 JSON、SSE 里最后一条；工具结果的文字、`isError`。

use serde_json::json;

use super::{last_message, text};
use crate::search::Failure;

#[test]
fn a_plain_json_answer_and_the_last_sse_message_are_read() {
    assert_eq!(
        last_message(r#" {"result": 1} "#),
        Some(json!({"result": 1}))
    );
    let sse = "event: message\ndata: {\"id\": 2, \"result\": {\"x\": 1}}\n\ndata: {\"method\": \"notifications/progress\"}\n\n";
    assert_eq!(
        last_message(sse),
        Some(json!({"id": 2, "result": {"x": 1}})),
        "排在后面的通知不算"
    );
    assert_eq!(last_message("data: not json"), None);
}

#[test]
fn the_text_parts_of_a_tool_result_are_joined_and_is_error_fails() {
    let ok = json!({"result": {"content": [{"type": "text", "text": "a"}, {"type": "image"}, {"type": "text", "text": "b"}]}});
    assert_eq!(text(&ok).unwrap(), "a\nb");
    let limited = json!({"result": {"isError": true, "content": [{"type": "text", "text": "Rate limit exceeded"}]}});
    assert_eq!(text(&limited), Err(Failure::Limited));
    let broken =
        json!({"result": {"isError": true, "content": [{"type": "text", "text": "bad query"}]}});
    assert_eq!(text(&broken), Err(Failure::Failed("bad query".to_string())));
}
