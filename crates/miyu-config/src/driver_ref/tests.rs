//! 扩展驱动的那个窄口子（施工 MD-1，core1 11-07 定 A）：只对 `providers.<id>.driver`、只对 `<包编号>/<驱动名>`，不在选项里的照留、
//! 记警告 `unknown_driver`；在选项里的照常；别的写法、别的格照旧 `not_an_option`。

use super::{PROVIDER_DRIVER, extension};
use crate::Layer;
use crate::Value;
use crate::parse::parse;
use crate::problem::{Code, Severity};
use crate::test_support::item;

fn text(value: &Value) -> Option<&str> {
    match value {
        Value::Text(text) => Some(text),
        _ => None,
    }
}

#[test]
fn only_package_slash_driver_is_the_extension_form() {
    for good in ["pi-antigravity/antigravity", "pi_x/a1", "p/x-y"] {
        assert!(extension(good), "{good}");
    }
    for bad in [
        "openai-chat",
        "Pi/antigravity",
        "pi/Anti",
        "pi/",
        "/x",
        "a/b/c",
        "pi/anti gravity",
        "",
    ] {
        assert!(!extension(bad), "{bad}");
    }
}

fn items() -> Vec<crate::Item> {
    vec![
        item(
            PROVIDER_DRIVER,
            &["openai-chat", "anthropic", "pi/ag"],
            "openai-chat",
        ),
        item("ui.language", &["zh", "en"], "zh"),
    ]
}

#[test]
fn an_extension_driver_not_listed_is_kept_with_a_warning() {
    let read = |text: &str| parse(&items(), Layer::System, text).expect("写法对");
    // 装着的：照常。
    let listed = read("[providers.ag]\ndriver = \"pi/ag\"\n");
    assert!(listed.problems.is_empty(), "{:?}", listed.problems);
    assert_eq!(
        text(&listed.entries["providers.ag.driver"].value),
        Some("pi/ag")
    );
    // 没装的：值照留，记一条警告，名字是写的那一个。
    let missing = read("[providers.ag]\ndriver = \"gone/ag\"\n");
    assert_eq!(
        text(&missing.entries["providers.ag.driver"].value),
        Some("gone/ag")
    );
    assert_eq!(missing.problems.len(), 1);
    let problem = &missing.problems[0];
    assert_eq!(problem.code, Code::UnknownDriver);
    assert_eq!(problem.code.severity(), Severity::Warning);
    assert_eq!(problem.name.as_deref(), Some("gone/ag"));
    // 别的写法照旧不收。
    for wrong in ["chatgpt", "a/b/c", "Gone/ag"] {
        let parsed = read(&format!("[providers.ag]\ndriver = \"{wrong}\"\n"));
        assert!(
            !parsed.entries.contains_key("providers.ag.driver"),
            "{wrong}"
        );
        assert_eq!(parsed.problems[0].code, Code::NotAnOption, "{wrong}");
    }
    // 别的格写成这个样子也照旧不收：口子只开在这一格。
    let other = read("[ui]\nlanguage = \"pi/ag\"\n");
    assert!(!other.entries.contains_key("ui.language"));
    assert_eq!(other.problems[0].code, Code::NotAnOption);
}
