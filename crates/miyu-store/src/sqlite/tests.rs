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

/// 再开一份有东西的库（施工 V-37 上）：查过、放掉查时读进来的页以后，照旧是原来那一份，读得回来、写得进。
#[test]
fn a_kept_database_still_reads_back_after_the_check() {
    let scratch = Scratch::bare();
    let root = scratch.path().join("root");
    fs::create_dir_all(&root).expect("建得了");
    fs::write(root.join(MARKER), "x").expect("写得进");
    let path = root.join("index/x.db");
    let (db, _) = open(&path, SCHEMA, 1);
    db.expect("开得了")
        .execute_batch(
            "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 5000)
             INSERT INTO t SELECT x FROM c",
        )
        .expect("写得进");
    let (db, opened) = open(&path, SCHEMA, 1);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    let db = db.expect("开得了");
    db.execute("INSERT INTO t (x) VALUES (5001)", [])
        .expect("写得进");
    let (count, sum): (i64, i64) = db
        .query_row("SELECT count(*), sum(x) FROM t", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("读得回来");
    assert_eq!((count, sum), (5001, 5001 * 5002 / 2));
}
