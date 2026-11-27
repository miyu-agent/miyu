//! 闲时合回（施工 V-28）：写过的库合一遍以后 `-wal` 是 0 字节、数据都在；SQLite 自己做完检查点以后下一次提交把 `-wal` 截回去
//! （`journal_size_limit`）；有过提交才等得到安静；放下了的库不再合；合完放掉页缓存以后库照常读写（施工 LW-15）。
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
    let kept = keep("test", &path, Mutex::new(db));
    (path, kept)
}

/// 合一遍的、要看 `-wal` 合没合的测试排着来（施工 CS-9）：关库的测试量了 `-wal` 再关，这期间别的测试合一遍会把它截成 0 字节。
pub(super) static MERGES: Mutex<()> = Mutex::new(());

/// 排着合一遍（[`MERGES`]）。
fn merge_all() -> Checkpointed {
    let _merging = MERGES.lock().unwrap_or_else(PoisonError::into_inner);
    checkpoint_all()
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
    let done = merge_all();
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
    let written = wal(&path);
    // 关上时不合（施工 CS-9）：-wal 原样留着（并着的测试先合过的是 0 字节），下次开库再合；合一遍不碰它，也不出错。
    let done = merge_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    assert_eq!(wal(&path), written);
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
    let done = merge_all();
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

/// 合完放掉页缓存（施工 LW-15）：放掉以后这个连接读到的还是原来那些行，接着写得进，再合一遍也照常。放掉了多少在这里看
/// 不到（SQLite 不经 SQL 交出页缓存的用量），量的结果在施工单 `LW-15-库闲下来放掉页缓存.md`。
#[test]
fn after_merging_lets_go_of_cached_pages_the_database_still_reads_and_writes() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| {
        write(conn, 0..400);
        assert_eq!(count(conn), 400, "读一遍，页都在缓存里");
    });
    let done = merge_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    with(&db, |conn| {
        assert_eq!(wal(&path), 0, "合回了");
        let body: String = conn
            .query_row("SELECT body FROM t WHERE x = 399", [], |row| row.get(0))
            .expect("读得了");
        assert_eq!(body, "x".repeat(2000), "放掉的页读回来还是那一行");
        write(conn, 400..410);
        assert_eq!(count(conn), 410, "接着写得进");
    });
    let done = merge_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    with(&db, |conn| assert_eq!(count(conn), 410, "再合一遍数据都在"));
}

/// 只读的连接（施工 LW-29）：登记以后合的那一遍不合它（它是只读的），只放它的页缓存；放掉以后照常读得到。
#[test]
fn a_reader_is_not_merged_and_still_reads_after_its_pages_are_let_go() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| write(conn, 0..300));
    let reader = Connection::open(&path).expect("开得了");
    reader
        .pragma_update(None, "query_only", true)
        .expect("设得了");
    let reader = keep_reader("test-reader", &path, Mutex::new(Some(reader)));
    with(&reader, |conn| {
        assert_eq!(count(conn), 300, "读一遍，页进了缓存")
    });
    let done = merge_all();
    assert!(done.failed.is_empty(), "{:?}", done.failed);
    assert!(done.shrunk >= 2, "写的、只读的都放了：{}", done.shrunk);
    with(&reader, |conn| {
        assert_eq!(count(conn), 300, "放掉以后照常读")
    });
    with(&db, |conn| {
        write(conn, 300..310);
        assert_eq!(count(conn), 310);
    });
    with(&reader, |conn| {
        assert_eq!(count(conn), 310, "读得到后来写的")
    });
}

/// 正被用着的连接不等（施工 LW-29）：放页缓存那一遍拿不到锁就跳过，不挡在那里。
#[test]
fn a_connection_in_use_is_skipped_not_waited_for() {
    let scratch = Scratch::bare();
    let (_, db) = kept(scratch.path());
    with(&db, |conn| write(conn, 0..5));
    let held = db.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(!Wal::shrink(&*db), "锁被拿着：跳过");
    drop(held);
    assert!(Wal::shrink(&*db), "放开以后放得了");
    let empty: Mutex<Option<Connection>> = Mutex::new(None);
    assert!(!Wal::shrink(&empty), "没开成的没有可放的");
}

/// 读了一次也等得到安静（施工 LW-29）：只读不写的那一阵过去以后，等着的一方照样醒来放页缓存。
#[test]
fn waiting_for_quiet_returns_after_a_read() {
    let (done, got) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        wait_quiet(Duration::from_millis(50));
        done.send(()).expect("送得出");
    });
    // 登记是一个进程一份：并着跑的别的测试里等着的一方会先醒、把这一声收走，所以隔一会儿再读一次，直到这一个等到。
    let mut waited = Err(mpsc::RecvTimeoutError::Timeout);
    for _ in 0..100 {
        read();
        waited = got.recv_timeout(Duration::from_millis(200));
        if waited.is_ok() {
            break;
        }
    }
    waited.expect("读过以后安静下来了");
    waiter.join().expect("没崩");
}

/// 刚安静下来只放页缓存、不合（施工 BO-3）：`-wal` 原样留着，库照旧读得到写过的。
#[test]
fn letting_go_of_cached_pages_leaves_the_wal_alone() {
    let scratch = Scratch::bare();
    let (path, db) = kept(scratch.path());
    with(&db, |conn| write(conn, 0..50));
    let before = wal(&path);
    assert!(before > 0, "写过的还在 -wal 里");
    assert!(shrink_all() >= 1, "这个连接放了一遍");
    assert_eq!(wal(&path), before, "没合、没截");
    with(&db, |conn| assert_eq!(count(conn), 50));
}

/// 接着等的那一阵里有过一声就马上交回假，不等到头（施工 BO-3）：一轮任务当中下一批提交来了，不合。
#[test]
fn a_write_while_staying_quiet_calls_the_merge_off() {
    let (done, got) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        done.send(stays_quiet(Duration::from_secs(600)))
            .expect("送得出");
    });
    // 登记是一个进程一份：并着跑的别的测试里等着的一方会把这一声收走，所以隔一会儿再叫一次，直到这一个看到。
    let mut stayed = Err(mpsc::RecvTimeoutError::Timeout);
    for _ in 0..100 {
        wrote();
        stayed = got.recv_timeout(Duration::from_millis(200));
        if stayed.is_ok() {
            break;
        }
    }
    assert_eq!(stayed, Ok(false), "有过一声的不合，也没有等满十分钟");
    waiter.join().expect("没崩");
}

/// 接着等的那一阵里一声都没有的交回真（施工 BO-3）。
#[test]
fn staying_quiet_to_the_end_lets_the_merge_go_ahead() {
    // 登记是一个进程一份：并着跑的别的测试随时会叫一声，所以多试几次，有一次安静到头就行。照时限试、不照次数（施工 CS-9：
    // 关库的测试并着写，两百次 1 毫秒常常都撞上；别的测试一秒以内写完，之后一试就安静到头）。
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let stayed =
        std::iter::from_fn(|| (std::time::Instant::now() < deadline).then_some(())).any(|()| {
            wait_quiet(Duration::ZERO);
            stays_quiet(Duration::from_millis(1))
        });
    assert!(stayed, "一声都没有的那一次安静到了头");
}
