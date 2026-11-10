//! 密钥项写错不抄原文（施工 SEC-1）：原文是什么样的照 JSON、TOML、人敲的几种认；配置文件里密钥项、密钥字段写成明文的，问题里
//! 没有原文、说成话也没有，只说几个字符的字；不是密钥的项、字段照旧照抄。

use crate::item::{Item, Kind, Layer};
use crate::parse::parse;
use crate::problem::{Code, Problem, Shape, holds_secret, secret_at, tell};
use crate::test_support::{item, secret_item, words};

/// 假的密钥：24 个字符。
const FAKE: &str = "sk-test-0123456789abcdef";

/// 一个密钥项、一个开关，和一组账号（备注、密钥两个字段）。
fn items() -> Vec<Item> {
    let manifest = crate::package::read(
        r#"[package]
name = { en = "Draw" }

[settings.accounts]
type = "list"
element = "record"
name = { en = "Accounts" }

[settings.accounts.fields.name]
type = "text"
name = { en = "Name" }

[settings.accounts.fields.key]
type = "secret"
name = { en = "Key" }
"#,
    )
    .expect("清单读得成");
    let mut items = vec![
        secret_item("say.mimo.key"),
        Item {
            kind: Kind::Bool,
            default: Some(crate::Value::Bool(false)),
            ..item("permission.start_read_only", &[], "")
        },
    ];
    items.extend(crate::package::settings::items(
        "draw",
        "draw",
        &manifest.settings,
    ));
    items
}

/// 读一份系统配置，交回问题。
fn problems(text: &str) -> Vec<Problem> {
    parse(&items(), Layer::System, text)
        .expect("TOML 写法对")
        .problems
}

/// 一条问题说成的话。
fn message(problem: &Problem) -> String {
    let items = items();
    tell(problem, &items, None, &words(&items))
        .expect("字齐全")
        .message
}

#[test]
fn the_shape_is_read_from_json_toml_or_plain_typing() {
    assert_eq!(
        Shape::of(&format!("\"{FAKE}\"")),
        Shape::Text(24),
        "JSON、TOML 的字"
    );
    assert_eq!(
        Shape::of(&format!("'{FAKE}'")),
        Shape::Text(24),
        "TOML 的字面字"
    );
    assert_eq!(Shape::of(FAKE), Shape::Text(24), "人敲的不带引号");
    assert_eq!(Shape::of("\"密钥\\n\""), Shape::Text(3), "转义去掉以后数");
    assert_eq!(Shape::of("[\"a\", \"b\"]"), Shape::List(2));
    assert_eq!(Shape::of("{\"key\": \"a\"}"), Shape::Table, "JSON 的对象");
    assert_eq!(Shape::of("{ key = \"a\" }"), Shape::Table, "TOML 的行内表");
    assert_eq!(Shape::of("12345"), Shape::Value(5));
    assert_eq!(
        Shape::of("\"a\"\nb = 1"),
        Shape::Text(9),
        "多出一行的不当成一个值"
    );
}

#[test]
fn only_kinds_and_keys_that_hold_a_secret_are_hidden() {
    let items = items();
    assert!(holds_secret(Kind::Secret));
    assert!(holds_secret(Kind::List(&Kind::Secret)));
    assert!(!holds_secret(Kind::Bool));
    assert!(secret_at(&items, "say.mimo.key"));
    assert!(secret_at(&items, "draw.accounts"), "整条带着密钥字段");
    assert!(secret_at(&items, "draw.accounts[1]"), "一项带着密钥字段");
    assert!(secret_at(&items, "draw.accounts[1].key"));
    assert!(!secret_at(&items, "draw.accounts[1].name"), "备注不是密钥");
    assert!(!secret_at(&items, "permission.start_read_only"));
    assert!(!secret_at(&items, "no.such.key"));
}

#[test]
fn a_plain_key_in_the_file_is_neither_kept_nor_said() {
    let found = problems(&format!("[say.mimo]\nkey = \"{FAKE}\"\n"));
    assert_eq!(found.len(), 1, "{found:?}");
    let problem = &found[0];
    assert_eq!(problem.code, Code::WrongType);
    assert_eq!(problem.got, None, "不记原文");
    assert_eq!(problem.shape, Some(Shape::Text(24)));
    assert!(
        !format!("{problem:?}").contains("0123456789"),
        "{problem:?}"
    );
    let said = message(problem);
    assert!(said.contains("写的是 24 个字符的字"), "{said}");
    assert!(!said.contains("0123456789"), "{said}");
    // 超过 80 个字的：截出来的前一段也不留，数的是整串。
    let long = format!("sk-test-{}", "0123456789abcdef".repeat(7));
    let found = problems(&format!("[say.mimo]\nkey = \"{long}\"\n"));
    assert_eq!(found[0].shape, Some(Shape::Text(120)));
    assert!(!message(&found[0]).contains("0123456789"));
}

#[test]
fn a_plain_key_in_a_record_field_is_hidden_and_other_fields_are_not() {
    let text = format!("[[draw.accounts]]\nname = 1\nkey = \"{FAKE}\"\n");
    let found = problems(&text);
    let at = |key: &str| {
        found
            .iter()
            .find(|problem| problem.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("报了 {key}：{found:?}"))
    };
    let key = at("draw.accounts[0].key");
    assert_eq!(
        (key.code, key.got.as_deref(), key.shape),
        (Code::WrongType, None, Some(Shape::Text(24)))
    );
    assert!(!message(key).contains("0123456789"));
    let name = at("draw.accounts[0].name");
    assert_eq!(
        (name.got.as_deref(), name.shape),
        (Some("1"), None),
        "备注照旧照抄"
    );
}

#[test]
fn an_unknown_key_or_field_never_keeps_its_value() {
    // 拼错的密钥项没有类型，值可能就是密钥：只记什么样的，最近的键名照给。
    let found = problems(&format!("[say.mimo]\nkye = \"{FAKE}\"\n"));
    let problem = &found[0];
    assert_eq!(problem.code, Code::UnknownKey);
    assert_eq!(
        (problem.got.as_deref(), problem.shape),
        (None, Some(Shape::Text(24)))
    );
    assert_eq!(problem.suggest, Some("say.mimo.key"));
    assert!(
        !format!("{problem:?}").contains("0123456789"),
        "{problem:?}"
    );
    let text =
        format!("[[draw.accounts]]\nname = \"A\"\nkey = {{ secret = \"a\" }}\nkyy = \"{FAKE}\"\n");
    let found = problems(&text);
    let field = found
        .iter()
        .find(|problem| problem.code == Code::UnknownField)
        .unwrap_or_else(|| panic!("报了不认识的字段：{found:?}"));
    assert_eq!(field.key.as_deref(), Some("draw.accounts[0].kyy"));
    assert_eq!(
        (field.got.as_deref(), field.shape),
        (None, Some(Shape::Text(24)))
    );
    assert!(!message(field).contains("0123456789"));
}

#[test]
fn a_value_where_a_group_belongs_never_keeps_its_value() {
    // 一组键写成一个值也没有类型：`say.mimo = "sk-…"` 照样只记什么样的。
    let found = problems(&format!("[say]\nmimo = \"{FAKE}\"\n"));
    let problem = &found[0];
    assert_eq!(
        (problem.code, problem.key.as_deref()),
        (Code::WrongType, Some("say.mimo"))
    );
    assert_eq!(
        (problem.got.as_deref(), problem.shape),
        (None, Some(Shape::Text(24)))
    );
    let said = message(problem);
    assert!(said.contains("写的是 24 个字符的字"), "{said}");
    assert!(!said.contains("0123456789"), "{said}");
}

#[test]
fn a_wrong_value_on_an_item_that_is_not_a_secret_is_still_echoed() {
    let found = problems("[permission]\nstart_read_only = \"yes\"\n");
    assert_eq!(
        (found[0].got.as_deref(), found[0].shape),
        (Some("\"yes\""), None)
    );
    assert!(message(&found[0]).contains("写的是 \"yes\""));
}
