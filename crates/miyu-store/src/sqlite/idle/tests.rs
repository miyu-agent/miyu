//! 闲时合回（施工 V-28）：写过的库合一遍以后 `-wal` 是 0 字节、数据都在；SQLite 自己做完检查点以后下一次提交把 `-wal` 截回去
//! （`journal_size_limit`）；有过提交才等得到安静；放下了的库不再合。
//!
//! 合一遍合的是这个进程里登记着的全部库，并着跑的别的测试也会合到这里的：量 `-wal` 的那几步拿着连接的锁做，合的一方排在后面。

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use super::super::open;
use super::*;
use crate::test_support::Scratch;

const SCHEMA: &str = "CREATE TABLE t (x INTEGER PRIMARY KEY, body TEXT NOT NULL)";

/// `path` 的 `-wal` 多大；不在的是 0。
fn wal(path: &Path) -> u64 {
    let mut name = path.as_os_str().to_owned();
    name.push("-wal");
    fs::metadata(PathBuf::from(name)).map_or(0, |meta| meta.len())
}

/// 在 `dir` 里开一份登记着的库。
fn kept(dir: &Path) -> (PathBuf, Arc<Mutex<Option<Connection>>>) {
    fs::create_dir_all(dir).expect("建得了");
    let path = dir.join("t.db");
    let (db, opened) = open(&path, SCHEMA, 1);
    assert!(db.is_some(), "{opened:?}");
    (path, keep("test", Mutex::new(db)))
}

/// 写 `rows` 行，每行一次提交。
fn write(db: &Connection, rows: std::ops::Range<i64>) {
    for x in rows {
        db.execute(
            "INSERT INTO t (x, body) VALUES (?1, ?2)",
            rusqlite::params![x, "x".repeat(2000)],
        )
        .expect("写得进");
    }
}

fn count(db: &Connection) -> i64 {
    db.query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("读得了")
}

/// 拿着连接的锁做 `body`。
fn with<R>(db: &Mutex<Option<Connection>>, body: impl FnOnce(&Connection) -> R) -> R {
    let guard = db.lock().expect("没崩");
    body(guard.as_ref().expect("开着"))
}

#[test]
fn a_written_wal_is_merged_back_and_cut_to_nothing() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| {
        write(conn, 0..50);
        assert!(wal(&path) > 0, "写过的在 -wal 里");
    });
    let done = checkpoint_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    with(&db, |conn| {
        assert_eq!(wal(&path), 0, "合回以后截成 0 字节");
        assert_eq!(count(conn), 50, "数据都在");
    });
    // 别的连接（同一个进程里 SQLite 自己开的）读得到：在主库里。
    let other = Connection::open(&path).expect("开得了");
    let rows: i64 = other
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("读得了");
    assert_eq!(rows, 50);
}

#[test]
fn after_sqlite_checkpoints_by_itself_the_next_commit_cuts_the_wal_back() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| {
        write(conn, 0..200);
        let high = wal(&path);
        assert!(high > 256 * 1024, "{high}");
        // SQLite 自己攒够了页做的那一种：被动的，不截。
        let busy: i64 = conn
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| row.get(0))
            .expect("合得了");
        assert_eq!(busy, 0);
        assert_eq!(wal(&path), high, "被动的检查点不截");
        write(conn, 200..201);
        assert!(
            wal(&path) < 64 * 1024,
            "下一次提交照 journal_size_limit 截回去：{}",
            wal(&path)
        );
        assert_eq!(count(conn), 201);
    });
}

#[test]
fn waiting_for_quiet_returns_after_a_write() {
    let scratch = Scratch::bare();
    let (_, db) = kept(scratch.path());
    let (tx, rx) = mpsc::channel();
    let waiting = std::thread::spawn(move || {
        wait_quiet(Duration::from_millis(20));
        tx.send(()).expect("还在等");
    });
    with(&db, |conn| write(conn, 0..3));
    rx.recv_timeout(Duration::from_secs(30))
        .expect("写过以后安静下来就交回");
    waiting.join().expect("没崩");
}

#[test]
fn a_dropped_database_is_no_longer_merged() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| write(conn, 0..5));
    let weak = Arc::downgrade(&db);
    drop(db);
    // 同一个测试进程里别的测试会并着合一遍全部库（`checkpoint_all`），合的时候临时把登记里的每一个升成强引用：撞上那一下
    // 这里会看到还活着。等它合完放开再判（V-28 修：本机连跑 30 遍红过 2 遍）。
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while weak.upgrade().is_some() {
        assert!(std::time::Instant::now() < deadline, "登记里只有弱引用");
        std::thread::sleep(Duration::from_millis(5));
    }
    // 关上时 SQLite 自己合回、删掉 -wal；合一遍不碰它，也不出错。
    let done = checkpoint_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    assert_eq!(wal(&path), 0);
}

/// 审计 2026-11-02 第三轮（边缘）：合是拿着写连接的锁做 `wal_checkpoint(TRUNCATE)`。有别的连接还在读老的一截（另一个进程开着库
/// 读、读的连接一条语句没收尾）时，TRUNCATE 照忙等的时限（rusqlite 开库默认 5 秒）一直等它读完，这期间这个库的写全排在后面；
/// 等不到的交回「没合完」，下一阵写以后再来一遍。
#[test]
fn a_reader_on_an_old_snapshot_does_not_hold_the_write_lock_for_seconds() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| write(conn, 0..5));
    let reader = Connection::open(&path).expect("开得了");
    reader.execute_batch("BEGIN").expect("开得了读");
    let rows: i64 = reader
        .query_row("SELECT COUNT(*) FROM t", [], |row| row.get(0))
        .expect("读得了");
    assert_eq!(rows, 5);
    // 读的那一个停在老的一截上。
    with(&db, |conn| write(conn, 5..10));
    let started = std::time::Instant::now();
    let done = checkpoint_all();
    let took = started.elapsed();
    reader.execute_batch("COMMIT").expect("收得了");
    assert!(
        took < Duration::from_secs(1),
        "合一遍等了 {took:?}（{done:?}），写连接的锁一直拿着"
    );
}

/// 合的时候把忙等时限临时设成 0（审计 R3E-4），合完照原来的设回去：平常的写照旧等别人的锁。
#[test]
fn merging_puts_the_busy_timeout_back() {
    let scratch = Scratch::bare();
    let (_, db) = kept(scratch.path());
    let timeout = |conn: &Connection| -> i64 {
        conn.query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("读得了")
    };
    // 拿着锁合：并着跑的别的测试合一遍也会合到这一个，先合掉了就成了「没什么要合的」。
    with(&db, |conn| {
        write(conn, 0..5);
        let before = timeout(conn);
        assert!(before > 0, "开库默认要等一会儿：{before}");
        assert_eq!(truncate(conn).expect("合得了"), Outcome::Done);
        assert_eq!(timeout(conn), before);
    });
}
