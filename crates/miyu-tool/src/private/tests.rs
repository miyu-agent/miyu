//! 照配置拼：没写的是出厂的；加了的、去掉的照写的；两项都过自己的校验。

use std::borrow::Cow;

use miyu_config::Value;

use super::*;

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn the_setting_adds_and_removes() {
    let home = Path::new("/home/alice");
    let none = Values::default();
    let quiet = |_: &str| None;
    assert_eq!(dirs_in(&none, home, &quiet), miyu_fs::private_dirs(home));
    let mut values = Values::default();
    values.set(ADD, Value::List(vec![text("~/diary")]));
    values.set(REMOVE, Value::List(vec![text(miyu_fs::SHIPPED_PRIVATE[0])]));
    let got = dirs_in(&values, home, &quiet);
    assert!(got.contains(&home.join("diary")));
    assert!(!got.contains(&home.join(".ssh")));
}

/// 私密目录整个关了（施工 5-17 上）：出厂的、加的都不藏；开着的（没写、写了开）照旧。
#[test]
fn the_switch_turns_them_all_off() {
    let home = Path::new("/home/alice");
    let quiet = |_: &str| None;
    let mut values = Values::default();
    values.set(ADD, Value::List(vec![text("~/diary")]));
    values.set(ENABLED, Value::Bool(false));
    assert!(dirs_in(&values, home, &quiet).is_empty());
    values.set(ENABLED, Value::Bool(true));
    assert!(dirs_in(&values, home, &quiet).contains(&home.join(".ssh")));
}
