//! 扩展的开关（施工 9-4 上）：没有的是空的；写的读回来一字不差；坏了的、版本不认识的说是坏了；这一瞬间有人改了的不写。

use std::fs;

use super::*;
use crate::test_support::Scratch;

#[test]
fn a_missing_file_has_no_switches_and_a_written_one_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join("system").join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert!(empty.switches.on.is_empty());
    assert_eq!(empty.version, None);
    let mut switches = empty.switches.clone();
    switches.on.insert("onebot".to_string(), true);
    switches.on.insert("echo".to_string(), false);
    write(&path, &switches, None).expect("写得进");
    let again = read(&path).expect("读得回");
    assert_eq!(again.switches, switches);
    assert!(again.version.is_some());
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"version\":1,\"on\":{\"echo\":false,\"onebot\":true}}\n"
    );
}

/// 批过的能力挪进组表以后（施工 Y-5 下上）：以前的文件里的 `approved` 认得、扔掉，开关照旧；写回时不再写这一格。
#[test]
fn an_old_approved_field_is_read_and_dropped() {
    let temp = Scratch::new();
    std::fs::create_dir_all(temp.path()).expect("建得了");
    let path = temp.path().join(FILE);
    fs::write(
        &path,
        "{\"version\":1,\"on\":{\"echo\":true},\"approved\":{\"echo\":[\"events.read\",\"network\"]}}\n",
    )
    .expect("写得进");
    let read_back = read(&path).expect("以前的文件读得进");
    assert_eq!(read_back.switches.on.get("echo"), Some(&true), "开关照旧");
    write(&path, &read_back.switches, read_back.version.as_deref()).expect("写得进");
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"version\":1,\"on\":{\"echo\":true}}\n",
        "写回时不再写 approved"
    );
}

#[test]
fn a_broken_or_unknown_file_is_reported() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    fs::create_dir_all(temp.path()).expect("建得了");
    for text in [
        "not json",
        "{\"version\":1}",
        "{\"version\":1,\"on\":{},\"later\":{}}",
        "{\"version\":1,\"on\":{\"onebot\":\"yes\"}}",
    ] {
        fs::write(&path, text).expect("写得进");
        assert!(
            matches!(read(&path), Err(BadFile::Shape(_))),
            "{text} 是坏的"
        );
    }
    fs::write(&path, "{\"version\":2,\"on\":{}}").expect("写得进");
    match read(&path) {
        Err(BadFile::Shape(said)) => assert_eq!(said, "unknown version 2"),
        other => panic!("版本不认识：{other:?}"),
    }
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    let switches = Switches::default();
    write(&path, &switches, None).expect("写得进");
    let first = read(&path).expect("读得回");
    let mut changed = switches.clone();
    changed.on.insert("onebot".to_string(), true);
    write(&path, &changed, first.version.as_deref()).expect("没人改过：写得进");
    let mut stale = switches;
    stale.on.insert("echo".to_string(), true);
    assert!(matches!(
        write(&path, &stale, first.version.as_deref()),
        Err(WriteError::Changed)
    ));
    assert_eq!(read(&path).expect("读得回").switches, changed, "没变");
}
