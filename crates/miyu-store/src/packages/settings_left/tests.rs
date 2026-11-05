//! 卸包留下的设置项的记号（施工 PK-5补）：记、读、删，写坏了的当前缀就是编号，不合写法的文件不算。

use super::*;
use crate::test_support::Scratch;

#[test]
fn marks_are_written_read_and_removed() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().join("home"));
    assert!(read(&root).is_empty(), "没有的是空的");
    mark(&root, "kit", "kt").expect("写得进");
    mark(&root, "echo", "echo").expect("写得进");
    assert!(has(&root, "kit") && !has(&root, "dog"));
    assert_eq!(
        read(&root),
        vec![
            ("echo".to_string(), "echo".to_string()),
            ("kit".to_string(), "kt".to_string())
        ]
    );
    assert_eq!(
        std::fs::read_to_string(root.leftover_settings("kit")).expect("读得到"),
        "prefix = \"kt\"\n"
    );
    unmark(&root, "kit").expect("删得掉");
    unmark(&root, "kit").expect("没有的不算错");
    assert!(!has(&root, "kit"));
    assert_eq!(read(&root).len(), 1);
}

#[test]
fn broken_marks_fall_back_to_the_id_and_strays_are_ignored() {
    let scratch = Scratch::new();
    let root = DataRoot::at(scratch.path().join("home"));
    let dir = root.leftover_settings_dir();
    std::fs::create_dir_all(dir.join("sub.toml")).expect("建得了");
    for (name, text) in [
        ("bad.toml", "prefix = ["),
        ("weird.toml", "prefix = \"Not Valid\""),
        ("Upper.toml", "prefix = \"x\""),
        ("plain", "prefix = \"x\""),
    ] {
        std::fs::write(dir.join(name), text).expect("写得进");
    }
    assert_eq!(
        read(&root),
        vec![
            ("bad".to_string(), "bad".to_string()),
            ("weird".to_string(), "weird".to_string())
        ]
    );
}
