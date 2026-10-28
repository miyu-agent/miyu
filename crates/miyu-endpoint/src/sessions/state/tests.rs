//! 查包的会话状态（施工 TW-1）：空的是清掉；只认三格；`note` 照给模型看的字查；`data` 有上限；`widgets` 只收声明了
//! `state = true` 的块，内容照那一块的位置查，错在哪的路径从 `widgets.<块>` 写起。

use serde_json::json;

use super::*;

/// 声明了两块：侧边栏的 `todos`、底栏的 `count`。
fn declared() -> Declared {
    Declared {
        widgets: vec![
            ("todos".to_string(), WidgetPlace::Sidebar),
            ("count".to_string(), WidgetPlace::Status),
        ],
        pages: Vec::new(),
    }
}

fn field(value: serde_json::Value) -> String {
    match read(&declared(), &value) {
        Err(Bad::Field(field)) => field,
        other => panic!("{value}: {other:?}"),
    }
}

#[test]
fn null_and_an_empty_object_clear() {
    assert!(read(&declared(), &json!(null)).expect("收").is_empty());
    assert!(read(&declared(), &json!({})).expect("收").is_empty());
    assert!(
        read(&declared(), &json!({"widgets": {}}))
            .expect("收")
            .is_empty()
    );
}

#[test]
fn every_field_is_kept_as_written() {
    let state = read(
        &declared(),
        &json!({
            "widgets": {
                "todos": {"content": [{"type": "list", "items": [{"title": "读代码", "state": "doing"}]}], "title": {"en": "Todos 0/1"}},
                "count": {"content": [{"type": "text", "text": "0/1"}]},
            },
            "note": "Todo list:\n- [ ] Read the code",
            "data": {"todos": [1, 2]},
        }),
    )
    .expect("收");
    assert_eq!(
        state.note.as_deref(),
        Some("Todo list:\n- [ ] Read the code")
    );
    assert_eq!(
        state.data.as_ref().map(|data| data.get()),
        Some(r#"{"todos":[1,2]}"#)
    );
    let widgets: serde_json::Value =
        serde_json::from_str(state.widgets.as_ref().expect("有").get()).expect("读得懂");
    assert_eq!(widgets["count"]["content"][0]["text"], "0/1");
}

#[test]
fn bad_fields_say_which() {
    assert_eq!(field(json!("x")), "state");
    assert_eq!(field(json!({"view": []})), "state.view");
    assert_eq!(field(json!({"note": "全是中文的一段说明"})), "state.note");
    assert_eq!(field(json!({"note": 3})), "state.note");
    assert_eq!(field(json!({"data": "x".repeat(DATA_MOST)})), "state.data");
    assert_eq!(field(json!({"widgets": []})), "state.widgets");
    assert_eq!(
        field(json!({"widgets": {"other": {"content": []}}})),
        "state.widgets.other",
        "没声明成状态块的"
    );
    assert_eq!(
        field(json!({"widgets": {"todos": {"content": [], "extra": 1}}})),
        "state.widgets.todos.extra"
    );
    assert_eq!(
        field(json!({"widgets": {"todos": {"title": {"en": "x"}}}})),
        "state.widgets.todos.content"
    );
}

#[test]
fn bad_content_is_checked_by_the_widget_place() {
    let bad = read(
        &declared(),
        &json!({"widgets": {"todos": {"content": [{"type": "nope"}]}}}),
    );
    let Err(Bad::Widget(bad)) = bad else {
        panic!("{bad:?}");
    };
    assert!(bad.path.starts_with("widgets.todos."), "{}", bad.path);
    // 底栏不收列表（`widgets.md`「内容」的位置限制）。
    let in_status = read(
        &declared(),
        &json!({"widgets": {"count": {"content": [{"type": "list", "items": [{"title": "x"}]}]}}}),
    );
    assert!(matches!(in_status, Err(Bad::Widget(_))), "{in_status:?}");
}
