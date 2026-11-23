//! 收回起点清单：两份都对才拷，有一份不对一份都不拷。

use super::*;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-order-adopt-{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(dir.join("from")).unwrap();
    fs::create_dir_all(dir.join("into")).unwrap();
    dir
}

#[test]
fn both_lists_are_copied_when_both_are_good() {
    let dir = scratch("good");
    fs::write(dir.join("from/macos-miyu.txt"), "main\nmiyu::main\n").unwrap();
    fs::write(dir.join("from/macos-miyu-tui.txt"), "main\n").unwrap();
    fs::write(dir.join("into/macos-miyu.txt"), "main\n").unwrap();
    let lines = adopt(&dir.join("from"), &dir.join("into")).unwrap();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("2 个名字（原来 1 个）"), "{lines:?}");
    assert_eq!(
        fs::read_to_string(dir.join("into/macos-miyu.txt")).unwrap(),
        "main\nmiyu::main\n"
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn nothing_is_copied_when_one_list_is_bad() {
    let dir = scratch("bad");
    fs::write(dir.join("from/macos-miyu.txt"), "main\n").unwrap();
    fs::write(dir.join("from/macos-miyu-tui.txt"), "main\nmain\n").unwrap();
    let error = adopt(&dir.join("from"), &dir.join("into")).unwrap_err();
    assert!(error.contains("一份都没拷"), "{error}");
    assert!(!dir.join("into/macos-miyu.txt").exists());
    assert!(adopt(&dir.join("nowhere"), &dir.join("into")).is_err());
    fs::remove_dir_all(&dir).unwrap();
}
