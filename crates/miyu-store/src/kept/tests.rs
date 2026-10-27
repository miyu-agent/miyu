//! 常驻的账号（施工 IM-1 下）：没有的是空的；记上、拿掉写回读得回；最后一个拿掉的包不写；版本不认识的说是坏了。

use std::fs;

use super::*;
use crate::test_support::Scratch;

#[test]
fn kept_accounts_are_written_and_the_last_one_removed_drops_the_package() {
    let temp = Scratch::new();
    let path = temp.path().join("system").join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert_eq!(empty.kept, Kept::default());
    let mut kept = empty.kept;
    kept.set("im", "alice", true);
    kept.set("im", "bob", true);
    kept.set("im", "bob", false);
    kept.set("other", "carol", false);
    assert!(kept.on("im", "alice"));
    assert!(!kept.on("im", "bob"));
    write(&path, &kept, None).expect("写得进");
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"version\":1,\"kept\":{\"im\":[\"alice\"]}}\n"
    );
    let again = read(&path).expect("读得回");
    assert_eq!(again.kept, kept);
    let mut kept = again.kept;
    kept.set("im", "alice", false);
    write(&path, &kept, again.version.as_deref()).expect("写得进");
    assert_eq!(
        fs::read_to_string(&path).expect("在"),
        "{\"version\":1,\"kept\":{}}\n"
    );
}

#[test]
fn an_unknown_version_is_bad() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).expect("建得了");
    let path = temp.path().join(FILE);
    fs::write(&path, "{\"version\":2,\"kept\":{}}\n").expect("写得进");
    assert!(matches!(read(&path), Err(BadFile::Shape(_))));
}
