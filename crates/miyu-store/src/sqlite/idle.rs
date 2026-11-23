//! 闲时把写前日志合回主库（施工 V-28，`internal/blueprint/store.md`「怎么走」的「库的写前日志」）。
//!
//! 库都是 WAL（`sqlite::connect`）：提交只往 `-wal` 后面追加，SQLite 攒到约 1000 页（约 4 MB）才自己做一次被动的检查点。
//! 核心一直开着，没合回的那一截就一直在 `-wal` 里；被杀、崩了、断电时它留在数据根里，下次开库才合。
//!
//! 一个进程一份登记：开着的写连接和只读的连接（弱引用，库放下了自己掉）、最后一次提交或者读的时刻。每次提交叫一声
//! （`sqlite::connect` 装的提交钩子，`wrote`），开库也算一声（上一回被杀留下的 `-wal` 也合），拿只读的连接读也算一声（`read`，
//! 施工 LW-29）。等着的一方（核心起的一个线程，`miyu-core` 的 `wal.rs`）照 [`wait_quiet`] 等到有过这样一声、之后这么久再
//! 没有，照 [`checkpoint_all`] 把每个 `-wal` 不空的库合回、截成 0 字节，再把每个登记着的连接的页缓存放掉，再回去等。
//!
//! 页缓存为什么每个连接都放（施工 LW-29）：SQLite 的每个连接把读过、写过的页留在自己的页缓存里，默认最多 2 MB，连接开着就
//! 一直占着。原来只放合过的库（施工 LW-15），只读的连接和这一阵只读没写的库不在里面：带着几个大的知识库、回合库、记忆库
//! 时，每个库读的那一个连接各留 2 MB。放的时候不等：正被用着的连接跳过，它用完会再叫一声，下一遍再放。
//!
//! - 没有写的时候等着的一方一直睡着，不定时醒；一阵写只叫醒它一次：它在等安静的时候，后来的提交只挪时刻、不叫。
//! - 合是 SQLite 自己的检查点（`PRAGMA wal_checkpoint(TRUNCATE)`）：先同步 `-wal`，抄回主库、同步主库，最后才截短。中途断电，
//!   下次开库照 `-wal` 接着合，不丢、不坏：崩溃安全和不合一样。
//! - 拿的是那个库的写连接的锁：同一个库的写排在它后面（一次几毫秒）；检索库读的连接不等（`recall/conn.rs`）。还有读在读老的
//!   一截的（别的进程开着它读），合不完、不截，交回「没合完」，等下一阵写以后再合，不为它再起一次。合的时候忙等的时限是 0：
//!   不等读的那一方读完，拿着锁干等（审计 R3E-4，原来一等就是开库默认的 5 秒）。
//! - 合的事不在异步运行时里做：等着的一方是一个自己的线程。

use std::ffi::OsString;
use std::fs;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, TryLockError, Weak};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use super::DbError;

/// 登记着的一个连接：哪一种库（写进运行日志，不写路径：路径里有人格的编号）、连接、是不是写的连接（只读的连接不合，
/// 闲下来只放页缓存，施工 LW-29）。
struct Kept {
    kind: &'static str,
    db: Weak<dyn Wal>,
    writes: bool,
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

/// 一个登记着的连接：合的时候、放页缓存的时候拿它的锁。
pub(crate) trait Wal: Send + Sync {
    /// 合回、截短：用不了的（没开成、关上了）、`-wal` 不在或者是空的什么都不做。
    fn truncate(&self) -> Result<Outcome, DbError>;

    /// 放掉这个连接攒着的页缓存（施工 LW-29）。正被别人用着的不等、交回假（它用完会再叫一声，下一遍再放）；没开成、
    /// 关上了的也交回假。
    fn shrink(&self) -> bool;
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
    /// 放了页缓存的连接有几个（施工 LW-29）：写的、只读的都算，正被用着没放成的不算。
    pub shrunk: usize,
}

impl Wal for Mutex<Option<Connection>> {
    fn truncate(&self) -> Result<Outcome, DbError> {
        match self.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            Some(db) => truncate(db),
            None => Ok(Outcome::Clean),
        }
    }

    fn shrink(&self) -> bool {
        match free(self) {
            Some(db) => db.as_ref().is_some_and(shrink),
            None => false,
        }
    }
}

impl Wal for Mutex<Connection> {
    fn truncate(&self) -> Result<Outcome, DbError> {
        truncate(&self.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn shrink(&self) -> bool {
        free(self).is_some_and(|db| shrink(&db))
    }
}

/// 这把锁现在没人拿着就拿：拿着的不等。别的线程拿着锁崩了的照常拿（库还是好的）。
fn free<T>(lock: &Mutex<T>) -> Option<MutexGuard<'_, T>> {
    match lock.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

/// 放掉一个连接的页缓存：之后用到哪一页再从系统的文件缓存读回来，结果不变。放不掉只是多占内存，交回假。
fn shrink(db: &Connection) -> bool {
    db.execute_batch("PRAGMA shrink_memory").is_ok()
}

/// 登记一个开着的写连接，`kind` 是哪一种库；交回包着它的那一份（登记里只留弱引用）。开库也算一次提交：上一回被杀留下的
/// `-wal` 等安静下来一起合。
pub(crate) fn keep<W: Wal + 'static>(kind: &'static str, db: W) -> Arc<W> {
    register(kind, db, true)
}

/// 登记一个开着的只读连接（施工 LW-29，检索库读的那一个）：不合它，库安静下来时只放它的页缓存。交回包着它的那一份。
pub(crate) fn keep_reader<W: Wal + 'static>(kind: &'static str, db: W) -> Arc<W> {
    register(kind, db, false)
}

fn register<W: Wal + 'static>(kind: &'static str, db: W, writes: bool) -> Arc<W> {
    let db = Arc::new(db);
    let weak: Weak<dyn Wal> = Arc::downgrade(&db) as Weak<dyn Wal>;
    let mut state = lock();
    state.open.retain(|kept| kept.db.strong_count() > 0);
    state.open.push(Kept {
        kind,
        db: weak,
        writes,
    });
    touch(&mut state);
    db
}

/// 提交了一次（提交钩子里调）：记下时刻；等着的一方还在等第一声的，叫醒它。
pub(crate) fn wrote() {
    touch(&mut lock());
}

/// 读了一次（拿只读连接的时候调，施工 LW-29）：和提交一样记下时刻、叫醒等着的一方。只读不写的那一阵（搜知识库、找记忆）
/// 过去以后，库照样等得到安静、放掉读进来的页。
pub(crate) fn read() {
    touch(&mut lock());
}

fn touch(state: &mut State) {
    if state.last.replace(Instant::now()).is_none() {
        WROTE.notify_all();
    }
}

/// 等到有过提交（开库、拿只读的连接读），之后 `quiet` 这么久再没有。没有的时候一直睡着；在等安静的时候只在到点时醒。
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

/// 把登记着的、还开着的库一个一个合回、截短（`PRAGMA wal_checkpoint(TRUNCATE)`）：`-wal` 不在、是空的跳过；只读的连接不合。
/// 每个登记着的连接再放一遍页缓存（施工 LW-29）。一次拿一个库的锁，合完放开。会等磁盘，在阻塞的线程里调。
pub fn checkpoint_all() -> Checkpointed {
    let open: Vec<(&'static str, Weak<dyn Wal>, bool)> = {
        let mut state = lock();
        state.open.retain(|kept| kept.db.strong_count() > 0);
        state
            .open
            .iter()
            .map(|kept| (kept.kind, Weak::clone(&kept.db), kept.writes))
            .collect()
    };
    let mut done = Checkpointed::default();
    for (kind, db, writes) in open {
        let Some(db) = db.upgrade() else {
            continue;
        };
        if writes {
            match db.truncate() {
                Ok(Outcome::Clean) => {}
                Ok(Outcome::Done) => done.done += 1,
                Ok(Outcome::Busy) => done.busy += 1,
                Err(error) => done.failed.push((kind, error)),
            }
        }
        // 每个连接的页缓存都放一遍（施工 LW-29）：只读的连接、这一阵只读没写的写连接没有东西要合，读进来的页照样留着
        // （默认每个连接最多 2 MB），原来只有合过的库才放。
        if db.shrink() {
            done.shrunk += 1;
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
    // 合的时候不忙等（审计 R3E-4）：TRUNCATE 碰上读老一截的，照忙等的时限（开库默认 5 秒）一直等它读完，这期间拿着写连接
    // 的锁，这个库的写全排在后面。时限设成 0，SQLite 就照被动的那样能抄的抄、交回没合完；合完设回原来的。
    let waited: i64 = db.query_row("PRAGMA busy_timeout", [], |row| row.get(0))?;
    db.busy_timeout(Duration::ZERO)?;
    let busy = db.query_row::<i64, _, _>("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0));
    db.pragma_update(None, "busy_timeout", waited)?;
    // 页缓存不在这里放：合完以后 [`checkpoint_all`] 对每个登记着的连接都放一遍（施工 LW-15 只放合过的，LW-29 改成都放）。
    Ok(if busy? == 0 {
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
