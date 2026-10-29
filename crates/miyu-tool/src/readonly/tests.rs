//! 照配置拼：没写的是出厂的；加了的、去掉的照写的。

use std::borrow::Cow;

use miyu_config::Value;

use super::*;

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn the_setting_adds_and_removes() {
    let home = Path::new("/home/alice");
    assert_eq!(
        list(&Values::default(), Some(home)),
        Readonly::shipped(Some(home))
    );
    let mut values = Values::default();
    values.set(ADD, Value::List(vec![text("Makefile")].into()));
    values.set(REMOVE, Value::List(vec![text(".vscode")].into()));
    let got = list(&values, Some(home));
    assert!(got.inside.contains(&vec!["Makefile".to_string()]));
    assert!(!got.inside.contains(&vec![".vscode".to_string()]));
    assert!(
        got.inside.contains(&vec![".idea".to_string()]),
        "别的出厂项照旧"
    );
}

#[test]
fn the_switch_turns_them_all_off() {
    let home = Path::new("/home/alice");
    let mut values = Values::default();
    values.set(ADD, Value::List(vec![text("Makefile")].into()));
    values.set(ENABLED, Value::Bool(true));
    assert!(
        list(&values, Some(home))
            .inside
            .contains(&vec!["Makefile".to_string()])
    );
    values.set(ENABLED, Value::Bool(false));
    assert_eq!(
        list(&values, Some(home)),
        Readonly::default(),
        "出厂的、加的都不生效"
    );
}

#[test]
fn the_switch_comes_first_in_its_group() {
    assert_eq!(ITEMS[0].key, ENABLED);
    assert!(ITEMS.iter().all(|item| item.ui.group == "readonly"));
}
