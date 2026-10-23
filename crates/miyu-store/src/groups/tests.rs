//! 组表（施工 Y-12 中下）：没有的是空的；写的读回来一字不差；加、移照组名，同一个人加两次只记一次，组空了整组拿掉；坏了的、
//! 版本不认识的说是坏了。

use std::fs;

use super::*;
use crate::test_support::Scratch;

#[test]
fn a_missing_file_has_no_groups_and_a_written_one_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join("system").join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert!(empty.groups.groups.is_empty());
    assert_eq!(empty.version, None);
    let mut groups = empty.groups.clone();
    assert!(groups.add(ADMIN, "alice"));
    assert!(groups.add("ext.tools", "miyu-render"));
    write(&path, &groups, None).expect("写得进");
    let again = read(&path).expect("读得回");
    assert_eq!(again.groups, groups);
    assert!(again.version.is_some());
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"version\":1,\"groups\":{\"admin\":[\"alice\"],\"ext.tools\":[\"miyu-render\"]}}\n"
    );
}

#[test]
fn members_are_added_once_and_an_empty_group_goes_away() {
    let mut groups = Groups::default();
    assert!(!groups.has(ADMIN, "bob"));
    assert!(groups.add(ADMIN, "bob"));
    assert!(!groups.add(ADMIN, "bob"), "加过的不再加");
    assert!(groups.add(ADMIN, "alice"));
    assert_eq!(groups.members(ADMIN), ["alice", "bob"], "照名字排");
    assert!(groups.has(ADMIN, "bob"));
    assert!(groups.remove(ADMIN, "bob"));
    assert!(!groups.remove(ADMIN, "bob"), "没有的移不了");
    assert!(groups.remove(ADMIN, "alice"));
    assert!(groups.groups.is_empty(), "组空了整组拿掉");
    assert!(groups.members("nobody").is_empty());
}

#[test]
fn a_broken_or_unknown_file_is_bad() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).expect("建得了");
    let path = temp.path().join(FILE);
    for text in [
        "not json",
        "{\"version\":2,\"groups\":{}}",
        "{\"version\":1,\"groups\":{},\"other\":1}",
        "{\"version\":1,\"groups\":{\"admin\":\"alice\"}}",
    ] {
        fs::write(&path, text).expect("写得进");
        assert!(read(&path).is_err(), "{text}");
    }
}
