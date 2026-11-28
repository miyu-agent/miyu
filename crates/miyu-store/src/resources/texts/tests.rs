//! 读好的给模型看的字放一阵（施工 PERF-5）：文件没变只读一遍，拷出来的、压了覆盖的共用；改了出厂的、加了改了删了覆盖
//! 下一次就是新的；压法不同的各放各的；读不出来的不放；放掉以后再读。

use std::fs;
use std::path::Path;

use crate::resources::ResourceRoot;
use crate::test_support::Scratch;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了目录");
    fs::write(path, text).expect("写得进去");
}

fn read(root: &ResourceRoot) -> String {
    root.read(&["core", "rule.txt"]).expect("读得出来")
}

#[test]
fn unchanged_texts_are_read_once_and_changes_are_read_again() {
    let scratch = Scratch::new();
    let shipped = scratch.path().join("resources/core/rule.txt");
    let mine = scratch.path().join("prompts");
    let overlay = mine.join("core/rule.txt");
    write(&shipped, "Shipped rule.\n");
    let root = ResourceRoot::at(scratch.path().join("resources")).overlaid(vec![mine]);
    assert_eq!(read(&root), "Shipped rule.\n");
    assert_eq!(read(&root), "Shipped rule.\n");
    // 拷出来的共用同一个架子。
    assert_eq!(read(&root.clone()), "Shipped rule.\n");
    assert_eq!(root.texts_read(), 1, "文件没变只读一遍");
    // 改了出厂的：核心不重启，下一次就是新的（同施工 W-1）。
    write(&shipped, "Shipped rule, a longer one.\n");
    assert_eq!(read(&root), "Shipped rule, a longer one.\n");
    // 加了覆盖、改了覆盖、删了覆盖，下一次都是新的。
    write(&overlay, "My own rule.\n");
    assert_eq!(read(&root), "My own rule.\n");
    write(&overlay, "My own rule, changed again.\n");
    assert_eq!(read(&root), "My own rule, changed again.\n");
    fs::remove_file(&overlay).expect("删得了");
    assert_eq!(read(&root), "Shipped rule, a longer one.\n");
    assert_eq!(root.texts_read(), 5, "变一次读一次");
    assert_eq!(read(&root), "Shipped rule, a longer one.\n");
    assert_eq!(root.texts_read(), 5, "又没变了，不再读");
    // 放掉以后再读。
    assert!(root.clone().forget_words(), "放着字");
    assert!(!root.forget_words(), "已经放掉了");
    assert_eq!(read(&root), "Shipped rule, a longer one.\n");
    assert_eq!(root.texts_read(), 6);
}

#[test]
fn overlays_are_kept_apart_and_failures_are_not_kept() {
    let scratch = Scratch::new();
    let shipped = scratch.path().join("resources/core/rule.txt");
    let mine = scratch.path().join("prompts");
    let plain = ResourceRoot::at(scratch.path().join("resources"));
    let overlaid = plain.overlaid(vec![mine.clone()]);
    assert!(plain.read(&["core", "rule.txt"]).is_err(), "还没有出厂的");
    write(&shipped, "Shipped rule.\n");
    write(&mine.join("core/rule.txt"), "My own rule.\n");
    assert_eq!(read(&plain), "Shipped rule.\n", "读不出来的没放着");
    assert_eq!(read(&overlaid), "My own rule.\n", "压了覆盖的各放各的");
    assert_eq!(read(&plain), "Shipped rule.\n");
    assert_eq!(read(&overlaid), "My own rule.\n");
    assert_eq!(overlaid.texts_read(), 3, "读不出来的那次也算，之后各读一次");
}
