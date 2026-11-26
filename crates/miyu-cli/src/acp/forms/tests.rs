use serde_json::json;

use super::{answers, message, schema, unanswered};
use crate::language::Language;
use miyu_kernel::event::Question;

/// 一道题。
fn question(text: &str, labels: &[&str], multiple: bool) -> Question {
    let options: Vec<_> = labels.iter().map(|label| json!({"label": label})).collect();
    serde_json::from_value(json!({"question": text, "options": options, "multiple": multiple}))
        .expect("是一道题")
}

#[test]
fn each_kind_of_question_gets_its_field() {
    let asked = [
        question("哪个？", &["a", "b"], false),
        question("哪些？", &["x", "y"], true),
        question("为什么？", &[], false),
    ];
    let form = schema(Language::Chinese, &asked);
    let fields = &form["properties"];
    assert_eq!(fields["q1"]["enum"], json!(["a", "b"]));
    // 几道题的：格的标题是题目，不写说明；自己写的那一格只写「其他」。
    assert_eq!(fields["q1"]["title"], "哪个？");
    assert!(fields["q1"].get("description").is_none());
    assert_eq!(
        fields["q1_other"],
        json!({"type": "string", "title": "其他"})
    );
    assert_eq!(fields["q2"]["items"]["enum"], json!(["x", "y"]));
    assert_eq!(fields["q3"]["type"], "string");
    assert!(fields["q3_other"].is_null(), "没有选项的题本来就是自己写");
    assert_eq!(message(Language::Chinese, &asked[..1]), "哪个？");
    // 一道题的：题目在表单上面，格里不再写一遍；有 header 的只写 header。
    let one = schema(Language::Chinese, &asked[..1]);
    assert!(one["properties"]["q1"].get("title").is_none(), "{one}");
    let mut headed = question("哪个？", &["a"], false);
    headed.header = Some("库".to_string());
    let one = schema(Language::Chinese, &[headed]);
    assert_eq!(one["properties"]["q1"]["title"], "库");
    let text = one.to_string();
    assert_eq!(
        text.matches("哪个？").count(),
        0,
        "题目一处都不在格里：{text}"
    );
    assert_eq!(message(Language::Chinese, &asked), "回答下面的问题");
}

#[test]
fn a_filled_form_is_read_question_by_question() {
    let asked = [
        question("哪个？", &["a", "b"], false),
        question("哪些？", &["x", "y", "z"], true),
        question("为什么？", &[], false),
    ];
    let content = json!({"q1": "b", "q1_other": "  ", "q2": ["z", "nope", "x", "z", 7],
        "q2_other": "还有 w", "q3": "因为"});
    assert_eq!(
        answers(&asked, &content),
        [
            json!({"picked": ["b"]}),
            json!({"picked": ["z", "x"], "text": "还有 w"}),
            json!({"text": "因为"}),
        ]
    );
    // 选的不是选项的不算。
    assert_eq!(answers(&asked, &json!({"q1": "c"}))[0], json!({}));
    assert_eq!(unanswered(&asked), [json!({}), json!({}), json!({})]);
}
