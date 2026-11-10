//! 包自己存过的 blob 的那一份（施工 BG-1）：追加了读得回，一个包一份，卸了删掉；坏行跳过、数得出几行；写到一半的最后一行
//! 不和下一行粘在一起；不合包编号写法的名字不认。

use std::fs;

use super::*;
use crate::test_support::Scratch;

fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

fn root(scratch: &Scratch) -> DataRoot {
    DataRoot::at(scratch.path().to_path_buf())
}

#[test]
fn what_was_appended_is_loaded_per_package() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    assert!(load(&root).unwrap().held.is_empty(), "目录还没有：空的");
    append(&root, "memes", &hash("one")).unwrap();
    append(&root, "memes", &hash("two")).unwrap();
    append(&root, "voice", &hash("three")).unwrap();
    assert_eq!(
        fs::read_to_string(root.blob_grants("memes")).unwrap(),
        format!("{}\n{}\n", hash("one").as_str(), hash("two").as_str()),
        "一行一个哈希"
    );
    let loaded = load(&root).unwrap();
    assert_eq!(
        loaded.held["memes"],
        BTreeSet::from([hash("one"), hash("two")])
    );
    assert_eq!(loaded.held["voice"], BTreeSet::from([hash("three")]));
    assert!(loaded.bad.is_empty() && loaded.unreadable.is_empty());
}

#[test]
fn a_forgotten_package_has_no_file_left() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    forget(&root, "memes").expect("本来就没有的不算错");
    append(&root, "memes", &hash("one")).unwrap();
    append(&root, "voice", &hash("two")).unwrap();
    forget(&root, "memes").unwrap();
    assert!(!root.blob_grants("memes").exists());
    let loaded = load(&root).unwrap();
    assert!(!loaded.held.contains_key("memes"));
    assert!(loaded.held.contains_key("voice"), "别的包的不动");
}

#[test]
fn bad_lines_are_skipped_and_counted() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    append(&root, "memes", &hash("one")).unwrap();
    let path = root.blob_grants("memes");
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"not a hash\n\n\xff\xfe\n");
    bytes.extend_from_slice(format!("{}\r\n", hash("two").as_str()).as_bytes());
    fs::write(&path, bytes).unwrap();
    let loaded = load(&root).unwrap();
    assert_eq!(
        loaded.held["memes"],
        BTreeSet::from([hash("one"), hash("two")]),
        "好的行照收，`\\r\\n` 结尾的也认"
    );
    assert_eq!(loaded.bad, vec![("memes".to_string(), 2)], "空行不算坏行");
}

#[test]
fn a_half_written_last_line_does_not_swallow_the_next_one() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    append(&root, "memes", &hash("one")).unwrap();
    let path = root.blob_grants("memes");
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"sha256:0123");
    fs::write(&path, bytes).unwrap();
    append(&root, "memes", &hash("two")).unwrap();
    let loaded = load(&root).unwrap();
    assert_eq!(
        loaded.held["memes"],
        BTreeSet::from([hash("one"), hash("two")])
    );
    assert_eq!(loaded.bad, vec![("memes".to_string(), 1)]);
}

#[test]
fn names_that_are_not_package_ids_and_unreadable_files_do_not_stop_the_rest() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    append(&root, "memes", &hash("one")).unwrap();
    let dir = root.blob_grants_dir();
    fs::write(dir.join("Not.An.Id"), format!("{}\n", hash("x").as_str())).unwrap();
    // 一个叫包编号的目录：读不成文件。
    fs::create_dir(dir.join("broken")).unwrap();
    let loaded = load(&root).unwrap();
    assert_eq!(loaded.held.len(), 1, "{:?}", loaded.held);
    assert_eq!(loaded.held["memes"], BTreeSet::from([hash("one")]));
    assert_eq!(loaded.unreadable.len(), 1);
    assert_eq!(loaded.unreadable[0].0, "broken");
}

#[test]
fn nothing_is_written_outside_a_data_root() {
    let scratch = Scratch::bare();
    let root = DataRoot::at(scratch.path().to_path_buf());
    assert!(append(&root, "memes", &hash("one")).is_err());
    assert!(!scratch.path().exists(), "数据根不在了的不建回来");
}
