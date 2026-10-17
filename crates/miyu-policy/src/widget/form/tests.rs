//! 整页里能改的两种段（施工 X-15 再补，设计 `33-界面扩展.md` 第十节）：`sections`、`form` 的每一格、值的类型、上限、只在整页、
//! 不嵌套、编号不重；照语言挑字挑到里面。

use miyu_config::package::WidgetPlace;
use serde_json::{Value, json};

use super::super::{Problem, check, localize};

/// 一张卡片：两种格都用上。
fn card(id: &str) -> Value {
    json!({
        "id": id,
        "title": {"zh": "测试群", "en": "Test group"},
        "subtitle": "123456",
        "badge": {"text": "禁言中", "tone": "warn"},
        "state": "good",
        "blocks": [
            {"type": "text", "text": "接话中"},
            {"type": "form", "id": format!("chat-{id}"), "fields": [
                {"key": "persona", "label": "人格", "control": "select", "value": null,
                 "options": [{"value": "miyu", "label": "Miyu"}, {"value": "dev", "label": {"zh": "开发", "en": "Dev"}}],
                 "default": {"text": "默认 · Miyu"}},
                {"key": "reply", "label": "接话", "hint": "关了只听不说", "control": "toggle", "value": true},
                {"key": "rate", "label": "限流", "control": "text", "value": null, "placeholder": "5/300s"},
                {"key": "font", "label": "字号", "control": "number", "value": 14, "min": 8, "max": 40, "integer": true},
                {"key": "words", "label": "触发词", "control": "list", "value": ["米优", "miyu"]},
                {"key": "extra", "label": "附加提示词", "control": "textarea", "value": "第一行\n第二行"}
            ], "submit": {"label": "保存", "call": "cards.save_one", "params": {"id": id}}},
            {"type": "buttons", "items": [{"label": "删掉", "action": {"call": "cards.remove", "params": {"id": id}}}]}
        ]
    })
}

/// 一整页：一组卡片，带「加一个」。
fn page() -> Value {
    json!([
        {"type": "heading", "text": "场所"},
        {"type": "sections", "items": [card("1"), card("2")],
         "add": {"label": "加一个群", "call": "cards.add",
                 "options": [{"value": "3", "label": "新群"}]}}
    ])
}

fn problem(content: &Value) -> (String, Problem) {
    let bad = check(WidgetPlace::Page, &[], content).expect_err("该拒");
    (bad.path, bad.problem)
}

/// 把 `page()` 里第一张卡的表单第 `field` 格换成 `with`。
fn field(field: usize, with: Value) -> Value {
    let mut page = page();
    page[1]["items"][0]["blocks"][1]["fields"][field] = with;
    page
}

#[test]
fn a_page_takes_sections_and_forms() {
    assert_eq!(check(WidgetPlace::Page, &[], &page()), Ok(()));
    for place in [WidgetPlace::Sidebar, WidgetPlace::Home] {
        let bad = check(place, &[], &page()).expect_err("只在整页里");
        assert_eq!(
            (bad.path.as_str(), bad.problem),
            ("content[1]", Problem::Place),
            "{place:?}"
        );
    }
    let alone = json!([{"type": "form", "id": "f", "fields": [], "submit": {"label": "存", "call": "save"}}]);
    assert_eq!(
        check(WidgetPlace::Page, &[], &alone),
        Ok(()),
        "表单也能直接放"
    );
    assert_eq!(
        check(WidgetPlace::Sidebar, &[], &alone).map_err(|bad| bad.problem),
        Err(Problem::Place)
    );
}

#[test]
fn each_mistake_in_a_form_says_where_and_what() {
    let at = "content[1].items[0].blocks[1].fields";
    let cases: Vec<(Value, String, Problem)> = vec![
        (
            field(
                1,
                json!({"key": "reply", "label": "接话", "control": "toggle", "value": "yes"}),
            ),
            format!("{at}[1].value"),
            Problem::Value,
        ),
        (
            field(
                0,
                json!({"key": "persona", "label": "人格", "control": "select", "value": "x",
                             "options": [{"value": "miyu", "label": "Miyu"}]}),
            ),
            format!("{at}[0].value"),
            Problem::Value,
        ),
        (
            field(
                0,
                json!({"key": "persona", "label": "人格", "control": "select", "value": null}),
            ),
            format!("{at}[0].options"),
            Problem::Missing,
        ),
        (
            field(
                1,
                json!({"key": "reply", "label": "接话", "control": "toggle", "value": true,
                             "options": [{"value": "a", "label": "A"}]}),
            ),
            format!("{at}[1].options"),
            Problem::Unknown,
        ),
        (
            field(
                2,
                json!({"key": "rate", "label": "限流", "control": "text", "value": null, "min": 1}),
            ),
            format!("{at}[2].min"),
            Problem::Unknown,
        ),
        (
            field(
                2,
                json!({"key": "rate", "label": "限流", "control": "slider", "value": null}),
            ),
            format!("{at}[2].control"),
            Problem::Type,
        ),
        (
            field(
                2,
                json!({"key": "rate", "label": "限流", "control": "text", "value": "x".repeat(201)}),
            ),
            format!("{at}[2].value"),
            Problem::TooLong,
        ),
        (
            field(
                5,
                json!({"key": "extra", "label": "附加", "control": "textarea", "value": "x".repeat(2001)}),
            ),
            format!("{at}[5].value"),
            Problem::TooLong,
        ),
        (
            field(
                4,
                json!({"key": "words", "label": "触发词", "control": "list", "value": ["a", 3]}),
            ),
            format!("{at}[4].value"),
            Problem::Value,
        ),
        (
            field(
                3,
                json!({"key": "font", "label": "字号", "control": "number", "value": "14"}),
            ),
            format!("{at}[3].value"),
            Problem::Value,
        ),
        (
            field(
                3,
                json!({"key": "words", "label": "字号", "control": "number", "value": 1}),
            ),
            format!("{at}[4].key"),
            Problem::Duplicate,
        ),
        (
            field(
                2,
                json!({"label": "限流", "control": "text", "value": null}),
            ),
            format!("{at}[2].key"),
            Problem::Missing,
        ),
    ];
    for (content, path, problem) in cases {
        assert_eq!(self::problem(&content), (path.clone(), problem), "{path}");
    }
}

/// 默认是什么（施工 X-15 三补）：`default.value` 可以不写；写了的照控件的类型查，下拉要是选项里的，`null` 不收。
#[test]
fn a_default_value_follows_the_control() {
    let at = "content[1].items[0].blocks[1].fields[0].default.value";
    let with = |default: Value| {
        field(
            0,
            json!({"key": "persona", "label": "人格", "control": "select", "value": null,
                   "options": [{"value": "miyu", "label": "Miyu"}], "default": default}),
        )
    };
    assert_eq!(
        check(
            WidgetPlace::Page,
            &[],
            &with(json!({"text": "默认 · Miyu", "value": "miyu"}))
        ),
        Ok(())
    );
    for wrong in [json!("dev"), json!(true), Value::Null] {
        assert_eq!(
            problem(&with(json!({"text": "默认", "value": wrong}))),
            (at.to_string(), Problem::Value),
            "{wrong}"
        );
    }
    let toggle = field(
        1,
        json!({"key": "reply", "label": "接话", "control": "toggle", "value": null,
               "default": {"text": "默认 · 开", "value": true}}),
    );
    assert_eq!(check(WidgetPlace::Page, &[], &toggle), Ok(()));
    let words = field(
        1,
        json!({"key": "reply", "label": "接话", "control": "toggle", "value": null,
               "default": {"text": "默认 · 开", "value": "on"}}),
    );
    assert_eq!(
        problem(&words),
        (
            "content[1].items[0].blocks[1].fields[1].default.value".to_string(),
            Problem::Value
        )
    );
}

#[test]
fn sections_are_one_per_block_never_nested_and_ids_do_not_repeat() {
    let mut nested = page();
    nested[1]["items"][0]["blocks"][0] = json!({"type": "sections", "items": []});
    assert_eq!(
        problem(&nested),
        ("content[1].items[0].blocks[0]".to_string(), Problem::Place)
    );
    let mut twice = page();
    twice
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "sections", "items": []}));
    assert_eq!(problem(&twice), ("content[2]".to_string(), Problem::Place));
    let mut same_item = page();
    same_item[1]["items"][1]["id"] = json!("1");
    assert_eq!(
        problem(&same_item),
        ("content[1].items[1].id".to_string(), Problem::Duplicate)
    );
    let mut same_form = page();
    same_form[1]["items"][1]["blocks"][1]["id"] = json!("chat-1");
    assert_eq!(
        problem(&same_form),
        (
            "content[1].items[1].blocks[1].id".to_string(),
            Problem::Duplicate
        )
    );
    let mut state = page();
    state[1]["items"][0]["state"] = json!("on");
    assert_eq!(
        problem(&state),
        ("content[1].items[0].state".to_string(), Problem::State)
    );
    let mut badge = page();
    badge[1]["items"][0]["badge"] = json!("x".repeat(21));
    assert_eq!(
        problem(&badge),
        ("content[1].items[0].badge".to_string(), Problem::TooLong)
    );
    let mut call = page();
    call[1]["add"]["call"] = json!("Bad Name");
    assert_eq!(
        problem(&call),
        ("content[1].add.call".to_string(), Problem::Action)
    );
    let mut submit = page();
    submit[1]["items"][0]["blocks"][1]["submit"]["call"] = json!("");
    assert_eq!(
        problem(&submit),
        (
            "content[1].items[0].blocks[1].submit.call".to_string(),
            Problem::Action
        )
    );
}

#[test]
fn too_many_items_fields_options_and_blocks_are_refused() {
    let mut items = page();
    items[1]["items"] = Value::Array(
        (0..201)
            .map(|n| json!({"id": n.to_string(), "title": "x", "blocks": []}))
            .collect(),
    );
    assert_eq!(
        problem(&items),
        ("content[1].items".to_string(), Problem::TooMany)
    );
    let mut blocks = page();
    blocks[1]["items"][0]["blocks"] = Value::Array(
        (0..33)
            .map(|_| json!({"type": "text", "text": "x"}))
            .collect(),
    );
    assert_eq!(
        problem(&blocks),
        ("content[1].items[0].blocks".to_string(), Problem::TooMany)
    );
    let mut fields = page();
    fields[1]["items"][0]["blocks"][1]["fields"] = Value::Array(
        (0..51)
            .map(|n| json!({"key": format!("k{n}"), "label": "x", "control": "toggle", "value": null}))
            .collect(),
    );
    assert_eq!(
        problem(&fields),
        (
            "content[1].items[0].blocks[1].fields".to_string(),
            Problem::TooMany
        )
    );
    let options: Vec<Value> = (0..101)
        .map(|n| json!({"value": n.to_string(), "label": "x"}))
        .collect();
    let content = field(
        0,
        json!({"key": "persona", "label": "人格", "control": "select", "value": null, "options": options}),
    );
    assert_eq!(
        problem(&content),
        (
            "content[1].items[0].blocks[1].fields[0].options".to_string(),
            Problem::TooMany
        )
    );
}

#[test]
fn words_inside_sections_and_forms_are_picked_too() {
    let picked = localize(&page(), "en");
    let card = &picked[1]["items"][0];
    assert_eq!(card["title"], "Test group");
    assert_eq!(card["blocks"][1]["fields"][0]["options"][1]["label"], "Dev");
    assert_eq!(
        card["badge"]["text"], "禁言中",
        "没给别的语言的照有的那一份"
    );
}
