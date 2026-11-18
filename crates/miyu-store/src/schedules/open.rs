//! 唤醒表怎么开（施工 J-1 上）：建表、查版本、查坏没坏照 [`crate::sqlite::connect`]，但它是真相，坏了的处理和派生的库不一样：
//!
//! - 读不了、坏了、版本比这一版旧又认不出的：连同 `-wal`、`-shm` 改名成 `schedules.db.broken-<毫秒>` 留着，开一份空的
//!   （[`Opened::SetAside`]）。人的定时没了，要告诉他（施工 J-3 接上系统通知，先记日志）。
//! - 版本比这一版新（新的程序写过、又换回旧的）：不动它，这一回不用（[`Opened::Newer`]），回到新的程序照样在。
//! - 写盘 `synchronous = FULL`：答应了的改动断电也在。锁住了不等，当场报错。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::sqlite::{self, DbError};

/// 表的版本。只加表、加索引的改动不升它。
pub(super) const VERSION: i64 = 1;

/// 建表。
const SCHEMA: &str = "
CREATE TABLE counter (next INTEGER NOT NULL);
INSERT INTO counter (next) VALUES (1);
CREATE TABLE schedules (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    by TEXT NOT NULL,
    when_json TEXT NOT NULL,
    prompt TEXT NOT NULL,
    target TEXT NOT NULL,
    target_session TEXT,
    loop TEXT,
    paused INTEGER NOT NULL DEFAULT 0,
    next_at INTEGER,
    last_at INTEGER,
    last_result TEXT,
    last_session TEXT
);
CREATE INDEX schedules_next ON schedules (next_at);
CREATE INDEX schedules_session ON schedules (target_session)";

/// 打开一份唤醒表是什么情形：调的一方照它记运行日志。
#[derive(Debug)]
pub enum Opened {
    /// 原来就有，版本对、查过没坏。
    Kept,
    /// 原来没有，新建了一份空的。
    Created,
    /// 原来的读不了、坏了：改名留着，开了一份空的。
    SetAside {
        /// 为什么。
        why: DbError,
        /// 改名以后的库文件。
        kept: PathBuf,
    },
    /// 原来的版本比这一版新：没动它，这一回不用。
    Newer(i64),
    /// 打不开（为什么）：这一回不用。
    Unusable(DbError),
}

/// 打开 `path`，没有就建。
pub(super) fn open(path: &Path) -> (Option<Connection>, Opened) {
    if let Some(found) = newer(path) {
        return (None, Opened::Newer(found));
    }
    let why = match connected(path) {
        Ok((db, created)) => {
            return (
                Some(db),
                if created {
                    Opened::Created
                } else {
                    Opened::Kept
                },
            );
        }
        Err(why) => why,
    };
    let kept = aside(path);
    match kept.and_then(|kept| Ok((connected(path)?, kept))) {
        Ok(((db, _), kept)) => (Some(db), Opened::SetAside { why, kept }),
        Err(error) => (None, Opened::Unusable(error)),
    }
}

/// 照共用的规矩开、建，再把写盘换成 `FULL`。
fn connected(path: &Path) -> Result<(Connection, bool), DbError> {
    let (db, created) = sqlite::connect(path, SCHEMA, VERSION)?;
    db.pragma_update(None, "synchronous", "FULL")?;
    // 锁住了不等：这个进程只有一个连接，别的进程开不了这个数据根（核心只有一个）；真锁住了（别人开着写）当场报错，
    // 由到点的那一边退避再试（端点 `schedules/hold.rs`），不在这里卡住后台任务。
    db.busy_timeout(std::time::Duration::ZERO)?;
    Ok((db, created))
}

/// 已有的库的版本比这一版新的，交回它的版本；没有库、读不出版本的不算（照常开，坏的改名留着）。只读地看，不建文件。
///
/// 开了又关会丢掉这个进程在这个库上的文件锁（`crate::sqlite` 的规矩）：只在这个进程还没开过它的时候看（只开一次）。
fn newer(path: &Path) -> Option<i64> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let found: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .ok()?;
    (found > VERSION).then_some(found)
}

/// 把 `path` 连同 `-wal`、`-shm` 改名成 `<名字>.broken-<毫秒>`，交回改好的库文件的位置。
fn aside(path: &Path) -> Result<PathBuf, DbError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let kept = suffixed(path, &format!(".broken-{stamp}"));
    for suffix in ["", "-wal", "-shm"] {
        match fs::rename(suffixed(path, suffix), suffixed(&kept, suffix)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(kept)
}

/// `path` 后面接上 `suffix`。
fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
