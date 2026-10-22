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
    values.set(ADD, Value::List(vec![text("Makefile")]));
    values.set(REMOVE, Value::List(vec![text(".vscode")]));
    let got = list(&values, Some(home));
    assert!(got.inside.contains(&vec!["Makefile".to_string()]));
    assert!(!got.inside.contains(&vec![".vscode".to_string()]));
    assert!(
        got.inside.contains(&vec![".idea".to_string()]),
        "别的出厂项照旧"
    );
}
