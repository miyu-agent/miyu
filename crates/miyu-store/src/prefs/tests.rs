//! 头自己的设置的文件（施工 HP-1）：没有的是空的；写的读回来一字不差；不是对象的当空的、带着版本能盖掉；这一瞬间有人改了的不写。

use std::fs;

use serde_json::json;

use super::*;
use crate::test_support::Scratch;

/// `value` 是一个对象：交回它的键值。
fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => panic!("要对象"),
    }
}

#[test]
fn a_missing_file_is_empty_and_a_written_one_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join("pane").join(FILE);
    assert_eq!(read(&path).expect("没有的是空的"), Read::default());
    let values = object(json!({"skin": "paper", "mascot": {"on": true}}));
    write(&path, &values, None).expect("写得进，目录建上");
    let again = read(&path).expect("读得回");
    assert_eq!(again.values, values);
    assert!(again.version.is_some());
    assert_eq!(again.bad, None);
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"mascot\":{\"on\":true},\"skin\":\"paper\"}\n"
    );
}

#[test]
fn not_an_object_reads_as_empty_and_is_overwritten_with_its_version() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).expect("建得了");
    let path = temp.path().join(FILE);
    fs::write(&path, "[1, 2]").expect("写得进");
    let bad = read(&path).expect("读得了");
    assert!(bad.values.is_empty());
    assert!(bad.bad.is_some(), "带着原话");
    let values = object(json!({"a": 1}));
    assert!(
        matches!(write(&path, &values, None), Err(WriteError::Changed)),
        "不带版本的不盖"
    );
    write(&path, &values, bad.version.as_deref()).expect("带着版本盖掉");
    assert_eq!(read(&path).expect("读得回").values, values);
}

#[test]
fn a_change_in_between_is_not_overwritten() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    write(&path, &object(json!({"a": 1})), None).expect("写得进");
    let seen = read(&path).expect("读得了");
    fs::write(&path, "{\"a\":2}\n").expect("手改");
    let refused = write(&path, &object(json!({"a": 3})), seen.version.as_deref());
    assert!(matches!(refused, Err(WriteError::Changed)), "{refused:?}");
    assert_eq!(read(&path).expect("读得了").values, object(json!({"a": 2})));
}
