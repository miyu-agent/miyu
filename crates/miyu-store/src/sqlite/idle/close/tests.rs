//! 停下时关库（施工 CS-9）：关的时候不合，`-wal` 留着、主库一个字节不动；下次开库读得到、只读地开也读得到、闲下来合回；
//! 合过的照常收掉 `-wal`、`-shm`；只关这个数据根里的；唤醒表那种关上以后读写报错、不碰文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags};

use super::super::super::{Opened, open};
use super::super::tests::MERGES;
use super::super::{checkpoint_all, keep};
use super::{close_under, open_under};
use crate::test_support::Scratch;

const SCHEMA: &str = "CREATE TABLE t (x INTEGER PRIMARY KEY, body TEXT NOT NULL)";

/// `path` 旁边的 `suffix`（`-wal`、`-shm`）。
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// 在 `dir` 里开一份登记着的库，写 `rows` 行（每行一次提交）。
fn written(dir: &Path, rows: i64) -> (PathBuf, Arc<Mutex<Option<Connection>>>) {
    fs::create_dir_all(dir).expect("建得了");
    let path = dir.join("t.db");
    let (db, opened) = open(&path, SCHEMA, 1);
    let db = db.unwrap_or_else(|| panic!("开得了：{opened:?}"));
    for x in 0..rows {
        db.execute(
            "INSERT INTO t (x, body) VALUES (?1, ?2)",
            rusqlite::params![x, "x".repeat(2000)],
        )
        .expect("写得进");
    }
    let kept = keep("test", &path, Mutex::new(Some(db)));
    (path, kept)
}

fn count(db: &Connection) -> i64 {
    db.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("读得了")
}

/// 关的时候不合（修之前最后一个连接关上时 SQLite 把 `-wal` 抄回主库、同步两次、删掉 `-wal`）：`-wal` 原样留着，主库一个
/// 字节不动；只读地开读得到全部；照规矩再开是原来那一份（不是删掉重建的），行都在；登记上以后合一遍，`-wal` 截成 0 字节。
#[test]
fn closing_leaves_the_wal_and_the_next_open_reads_and_merges_it() {
    let _merges = MERGES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let scratch = Scratch::bare();
    let (path, db) = written(scratch.path(), 30);
    let wal = fs::metadata(beside(&path, "-wal")).expect("在").len();
    assert!(wal > 0, "没合的在 -wal 里");
    let main = fs::read(&path).expect("读得了");
    let closed = close_under(scratch.path());
    assert!(closed.failed.is_empty(), "{:?}", closed.failed);
    assert_eq!(closed.closed, 1);
    assert!(db.lock().expect("没崩").is_none(), "拿走了");
    assert_eq!(open_under(scratch.path()), 0, "这个数据根里没有开着的库");
    assert_eq!(
        fs::metadata(beside(&path, "-wal")).expect("留着").len(),
        wal,
        "关的时候不合"
    );
    assert_eq!(fs::read(&path).expect("读得了"), main, "主库一个字节不动");
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let reader = Connection::open_with_flags(&path, flags).expect("只读地开得了");
    assert_eq!(count(&reader), 30, "只读地开读得到 -wal 里的");
    drop(reader);
    let (again, opened) = open(&path, SCHEMA, 1);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    let again = keep("test", &path, Mutex::new(again));
    let done = checkpoint_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    let guard = again.lock().expect("没崩");
    assert_eq!(count(guard.as_ref().expect("开着")), 30, "行都在");
    assert_eq!(
        fs::metadata(beside(&path, "-wal")).map_or(0, |meta| meta.len()),
        0,
        "下次开库合回"
    );
}

/// 合过的（`-wal` 是空的）关的时候照常收掉 `-wal`、`-shm`：好好退出不多留两个文件。
#[test]
fn closing_a_merged_database_removes_its_wal_and_shm() {
    let _merges = MERGES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let scratch = Scratch::bare();
    let (path, db) = written(scratch.path(), 3);
    {
        let guard = db.lock().expect("没崩");
        let busy: i64 = guard
            .as_ref()
            .expect("开着")
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
            .expect("合得了");
        assert_eq!(busy, 0);
    }
    assert!(beside(&path, "-shm").exists(), "开着时有 -shm");
    let closed = close_under(scratch.path());
    assert!(closed.failed.is_empty(), "{:?}", closed.failed);
    assert!(!beside(&path, "-wal").exists(), "空的 -wal 收掉了");
    assert!(!beside(&path, "-shm").exists(), "-shm 收掉了");
    let (again, opened) = open(&path, SCHEMA, 1);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    assert_eq!(count(&again.expect("开得了")), 3);
}

/// 只关这个数据根里的：同一个进程里别的核心的库照旧开着、照常写。
#[test]
fn closing_one_root_leaves_another_open() {
    let (one, other) = (Scratch::bare(), Scratch::bare());
    let (_, closing) = written(one.path(), 1);
    let (_, staying) = written(other.path(), 1);
    let closed = close_under(one.path());
    assert_eq!(closed.closed, 1);
    assert!(closing.lock().expect("没崩").is_none());
    assert_eq!(open_under(other.path()), 1);
    let guard = staying.lock().expect("没崩");
    let db = guard.as_ref().expect("照旧开着");
    db.execute("INSERT INTO t (x, body) VALUES (9, 'y')", [])
        .expect("照常写");
    assert_eq!(count(db), 2);
}

/// 一直开着、拿不走的那种（唤醒表，`Mutex<Connection>`）：关上以后换成内存里的空库，再来的读写报错、库文件不变。
#[test]
fn a_kept_connection_errs_after_closing_and_leaves_the_file_alone() {
    let _merges = MERGES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let scratch = Scratch::bare();
    fs::create_dir_all(scratch.path()).expect("建得了");
    let path = scratch.path().join("s.db");
    let (db, opened) = open(&path, SCHEMA, 1);
    let db = keep(
        "test",
        &path,
        Mutex::new(db.unwrap_or_else(|| panic!("{opened:?}"))),
    );
    db.lock()
        .expect("没崩")
        .execute("INSERT INTO t (x, body) VALUES (1, 'a')", [])
        .expect("写得进");
    assert_eq!(open_under(scratch.path()), 1);
    let closed = close_under(scratch.path());
    assert!(closed.failed.is_empty(), "{:?}", closed.failed);
    assert_eq!(open_under(scratch.path()), 0);
    let wal = fs::metadata(beside(&path, "-wal")).expect("留着").len();
    let refused = db
        .lock()
        .expect("没崩")
        .execute("INSERT INTO t (x, body) VALUES (2, 'b')", []);
    assert!(refused.is_err(), "关上以后写不进：{refused:?}");
    assert_eq!(fs::metadata(beside(&path, "-wal")).expect("在").len(), wal);
}
