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
    assert_eq!(dirs(&none, home), miyu_fs::private_dirs(home));
    let mut values = Values::default();
    values.set(ADD, Value::List(vec![text("~/diary")]));
    values.set(REMOVE, Value::List(vec![text(miyu_fs::SHIPPED_PRIVATE[0])]));
    let got = dirs(&values, home);
    assert!(got.contains(&home.join("diary")));
    assert!(!got.contains(&home.join(".ssh")));
}
