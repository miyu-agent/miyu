//! 闲时把写前日志合回主库（施工 V-28，`docs/blueprint/store.md`「怎么走」的「库的写前日志」）。
//!
//! 库都是 WAL（`sqlite::connect`）：提交只往 `-wal` 后面追加，SQLite 攒到约 1000 页（约 4 MB）才自己做一次被动的检查点。
//! 核心一直开着，没合回的那一截就一直在 `-wal` 里；被杀、崩了、断电时它留在数据根里，下次开库才合。
//!
//! 一个进程一份登记：开着的写连接（弱引用，库放下了自己掉）、最后一次提交的时刻。每次提交叫一声（`sqlite::connect` 装的
//! 提交钩子，`wrote`），开库也算一声（上一回被杀留下的 `-wal` 也合）。等着的一方（核心起的一个线程，`miyu-core` 的
//! `wal.rs`）照 [`wait_quiet`] 等到有过提交、之后这么久再没有提交，照 [`checkpoint_all`] 把每个 `-wal` 不空的库合回、截成
//! 0 字节，再回去等。
//!
//! - 没有写的时候等着的一方一直睡着，不定时醒；一阵写只叫醒它一次：它在等安静的时候，后来的提交只挪时刻、不叫。
//! - 合是 SQLite 自己的检查点（`PRAGMA wal_checkpoint(TRUNCATE)`）：先同步 `-wal`，抄回主库、同步主库，最后才截短。中途断电，
//!   下次开库照 `-wal` 接着合，不丢、不坏：崩溃安全和不合一样。
//! - 拿的是那个库的写连接的锁：同一个库的写排在它后面（一次几毫秒）；检索库读的连接不等（`recall/conn.rs`）。还有读在读老的
//!   一截的（别的进程开着它读），合不完、不截，交回「没合完」，等下一阵写以后再合，不为它再起一次。
//! - 合的事不在异步运行时里做：等着的一方是一个自己的线程。

use std::ffi::OsString;
use std::fs;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::DbError;

/// 登记着的一个写连接：哪一种库（写进运行日志，不写路径：路径里有人格的编号）、连接。
struct Kept {
    kind: &'static str,
    db: Weak<dyn Wal>,
}

/// 一个进程的登记。
struct State {
    open: Vec<Kept>,
    /// 最后一次提交（开库）的时刻；等着的一方合过以后清掉。
    last: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    open: Vec::new(),
    last: None,
});

/// 有过提交了：叫醒等着的一方。
static WROTE: Condvar = Condvar::new();

/// 一个写连接：合的时候拿它的锁。
pub(crate) trait Wal: Send + Sync {
    /// 合回、截短：用不了的（没开成、关上了）、`-wal` 不在或者是空的什么都不做。
    fn truncate(&self) -> Result<Outcome, DbError>;
}

/// 合一个库的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// 没什么要合的。
    Clean,
    /// 合回了、截成了 0 字节。
    Done,
    /// 有读的在读老的一截：能抄的抄了，没截。
    Busy,
}

/// 合了一遍（[`checkpoint_all`]）。
#[derive(Debug, Default)]
pub struct Checkpointed {
    /// 合回、截短了的库有几个。
    pub done: usize,
    /// 有读的在读、没合完的有几个：等下一阵写以后再合。
    pub busy: usize,
    /// 合不了的：哪一种库（`sessions`、`usage`、`rates`、`schedules`、`recall`），为什么。
    pub failed: Vec<(&'static str, DbError)>,
}

impl Wal for Mutex<Option<Connection>> {
    fn truncate(&self) -> Result<Outcome, DbError> {
        match self.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            Some(db) => truncate(db),
            None => Ok(Outcome::Clean),
        }
    }
}

impl Wal for Mutex<Connection> {
    fn truncate(&self) -> Result<Outcome, DbError> {
        truncate(&self.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// 登记一个开着的写连接，`kind` 是哪一种库；交回包着它的那一份（登记里只留弱引用）。开库也算一次提交：上一回被杀留下的
/// `-wal` 等安静下来一起合。
pub(crate) fn keep<W: Wal + 'static>(kind: &'static str, db: W) -> Arc<W> {
    let db = Arc::new(db);
    let weak: Weak<dyn Wal> = Arc::downgrade(&db) as Weak<dyn Wal>;
    let mut state = lock();
    state.open.retain(|kept| kept.db.strong_count() > 0);
    state.open.push(Kept { kind, db: weak });
    touch(&mut state);
    db
}

/// 提交了一次（提交钩子里调）：记下时刻；等着的一方还在等第一声的，叫醒它。
pub(crate) fn wrote() {
    touch(&mut lock());
}

fn touch(state: &mut State) {
    if state.last.replace(Instant::now()).is_none() {
        WROTE.notify_all();
    }
}

/// 等到有过提交（开库），之后 `quiet` 这么久再没有提交。没有提交的时候一直睡着；在等安静的时候只在到点时醒。
pub fn wait_quiet(quiet: Duration) {
    let mut state = lock();
    loop {
        let Some(last) = state.last else {
            state = WROTE.wait(state).unwrap_or_else(PoisonError::into_inner);
            continue;
        };
        let due = last + quiet;
        let now = Instant::now();
        if now >= due {
            state.last = None;
            return;
        }
        state = WROTE
            .wait_timeout(state, due - now)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

/// 把登记着的、还开着的库一个一个合回、截短（`PRAGMA wal_checkpoint(TRUNCATE)`）：`-wal` 不在、是空的跳过。一次拿一个库的锁，
/// 合完放开。会等磁盘，在阻塞的线程里调。
pub fn checkpoint_all() -> Checkpointed {
    let open: Vec<(&'static str, Weak<dyn Wal>)> = {
        let mut state = lock();
        state.open.retain(|kept| kept.db.strong_count() > 0);
        state
            .open
            .iter()
            .map(|kept| (kept.kind, Weak::clone(&kept.db)))
            .collect()
    };
    let mut done = Checkpointed::default();
    for (kind, db) in open {
        let Some(db) = db.upgrade() else {
            continue;
        };
        match db.truncate() {
            Ok(Outcome::Clean) => {}
            Ok(Outcome::Done) => done.done += 1,
            Ok(Outcome::Busy) => done.busy += 1,
            Err(error) => done.failed.push((kind, error)),
        }
    }
    done
}

/// 合回、截短一个库：`-wal` 不在、是空的不碰（只看一眼文件的大小，不拿 SQLite 的锁）。
fn truncate(db: &Connection) -> Result<Outcome, DbError> {
    let Some(path) = db.path().filter(|path| !path.is_empty()) else {
        return Ok(Outcome::Clean);
    };
    let mut wal = OsString::from(path);
    wal.push("-wal");
    match fs::metadata(&wal) {
        Ok(meta) if meta.len() > 0 => {}
        Ok(_) => return Ok(Outcome::Clean),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Outcome::Clean),
        Err(error) => return Err(error.into()),
    }
    let busy: i64 = db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    Ok(if busy == 0 {
        Outcome::Done
    } else {
        Outcome::Busy
    })
}

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
