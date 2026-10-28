//! 开库时建不建目录（施工 TMP-1）：数据根还在的照建；数据根被删了的不建回来、这一回用不了、错误里带着目录；目录本来就有的
//! 不查标记。

use super::*;
use crate::root::MARKER;
use crate::test_support::Scratch;

const SCHEMA: &str = "CREATE TABLE t (x INTEGER)";

#[test]
fn a_database_is_created_inside_a_data_root() {
    let scratch = Scratch::bare();
    let root = scratch.path().join("root");
    fs::create_dir_all(&root).expect("建得了");
    fs::write(root.join(MARKER), "x").expect("写得进");
    let (db, opened) = open(&root.join("home/a/index/x.db"), SCHEMA, 1);
    assert!(db.is_some(), "{opened:?}");
    assert!(matches!(opened, Opened::Created));
}

#[test]
fn a_deleted_data_root_is_not_brought_back() {
    let scratch = Scratch::bare();
    let gone = scratch.path().join("gone");
    let (db, opened) = open(&gone.join("home/a/index/x.db"), SCHEMA, 1);
    assert!(db.is_none());
    match opened {
        Opened::Unusable(error) => assert!(
            error.to_string().contains("not inside a Miyu data root"),
            "{error}"
        ),
        other => panic!("{other:?}"),
    }
    assert!(!gone.exists(), "一层都不建");
}

#[test]
fn an_existing_folder_needs_no_marker() {
    let scratch = Scratch::bare();
    let dir = scratch.path().join("plain");
    fs::create_dir_all(&dir).expect("建得了");
    let (db, opened) = open(&dir.join("x.db"), SCHEMA, 1);
    assert!(db.is_some(), "{opened:?}");
}
