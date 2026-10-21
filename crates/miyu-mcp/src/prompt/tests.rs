//! 提示词照写法读（施工 X-39）：名字、给人看的名字、说明、参数；没名字、参数没名字的不收，名字带空白的照原样收；消息照 `role` 收。

use serde_json::json;

use super::*;

#[test]
fn a_prompt_is_read_with_its_arguments() {
    let prompt = Prompt::read(&json!({
        "name": "review",
        "title": "  ",
        "description": "Review.",
        "arguments": [{"name": "code", "required": true}, {"name": "focus", "required": "yes"}],
    }))
    .expect("合写法");
    assert_eq!(prompt.name, "review");
    assert_eq!(prompt.title, None, "空白的给人看的名字当没有");
    assert_eq!(prompt.description.as_deref(), Some("Review."));
    let required: Vec<bool> = prompt
        .arguments
        .iter()
        .map(|argument| argument.required)
        .collect();
    assert_eq!(required, [true, false], "只认 true");
}

#[test]
fn a_badly_written_prompt_is_not_taken() {
    assert_eq!(
        Prompt::read(&json!({"name": "two words"})).map(|prompt| prompt.name),
        Ok("two words".to_string()),
        "名字照原样，清理是核心的事"
    );
    for raw in [
        json!({}),
        json!({"name": ""}),
        json!({"name": "x", "arguments": [{"description": "no name"}]}),
    ] {
        assert!(Prompt::read(&raw).is_err(), "{raw}");
    }
}

#[test]
fn messages_are_read_by_role() {
    let messages = Message::read_all(&json!({"messages": [
        {"role": "user", "content": {"type": "text", "text": "hi"}},
        {"role": "system", "content": {"type": "text", "text": "no"}},
        {"role": "assistant"},
        {"role": "assistant", "content": {"type": "text", "text": "ok"}},
    ]}));
    assert_eq!(
        messages,
        [
            Message {
                role: Role::User,
                content: Content::Text("hi".to_string())
            },
            Message {
                role: Role::Assistant,
                content: Content::Text("ok".to_string())
            },
        ]
    );
    assert!(Message::read_all(&json!({})).is_empty());
}
