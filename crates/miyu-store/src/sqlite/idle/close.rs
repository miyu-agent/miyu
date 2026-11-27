//! 核心停下时关库（施工 CS-9，`internal/blueprint/store.md`「怎么走」的「库的写前日志」第 5 款）：放数据根的锁以前，把这个
//! 数据根里登记着的库一个个关上，放了锁以后进程里不再有开着的库，也就不再碰数据根里的库文件。
//!
//! 关的时候不合：每个连接开库时设了 `SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE`（`sqlite::keep_wal_on_close`），没合的留在 `-wal` 里，
//! 下次开库照常读到、闲下来合回。`-wal` 已经是空的（闲时合过、开了没写过）的，关之前把这一项设回去：SQLite 照常删掉 `-wal`、
//! `-shm`，空的 `-wal` 没有要抄的、不同步（2026-11-27 用 strace 数过：0 次）。好好退出、库都合过的数据根里不多留两个文件。
//!
//! 一个进程一份登记，测试里一个进程开好几个核心：只关库文件在这个数据根里的，别的核心的不碰。

use std::path::Path;

use rusqlite::Connection;
use rusqlite::config::DbConfig;

use super::{DbError, opened, wal_written};

/// 关了一遍（[`close_under`]）。
#[derive(Debug, Default)]
pub struct Closed {
    /// 关上了的连接有几个：写的、只读的都算。
    pub closed: usize,
    /// 关的时候出了错的：哪一种库，为什么。连接照样放下了。
    pub failed: Vec<(&'static str, DbError)>,
}

/// 关上库文件在 `root` 里的、登记着的连接（写的、只读的）：正被用着的等它用完。关上以后，那一份读出来什么都没有、什么都
/// 不写（唤醒表换成内存里的空库，之后的读写报错）。会等磁盘，在阻塞的线程里调。
pub fn close_under(root: &Path) -> Closed {
    let mut done = Closed::default();
    for (kind, db, _, _) in opened(|path| path.starts_with(root)) {
        match db.close() {
            Ok(()) => done.closed += 1,
            Err(error) => done.failed.push((kind, error)),
        }
    }
    done
}

/// 登记着的、还开着的连接里，库文件在 `root` 里的有几个：测试看停下以后关干净了没有。关上了的（拿走了连接的）也还在登记里，
/// 照它是不是还开着数。
#[cfg(any(test, feature = "testkit"))]
pub fn open_under(root: &Path) -> usize {
    opened(|path| path.starts_with(root))
        .iter()
        .filter(|(_, db, _, _)| db.is_open())
        .count()
}

/// 关上一个连接：`-wal` 没东西的照常让 SQLite 收掉 `-wal`、`-shm`（见这一页开头）。关不上的连接也放下，交回为什么。
pub(super) fn close(db: Connection) -> Result<(), DbError> {
    if !wal_written(&db).unwrap_or(true) {
        db.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)?;
    }
    db.close().map_err(|(_, error)| DbError::Sql(error))
}

#[cfg(test)]
mod tests;
