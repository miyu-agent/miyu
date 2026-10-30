//! 唤醒表（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 1 条）：一份 `state/schedules.db`，一行一个到点
//! 叫她跑一轮的任务。定时任务、固定间隔的循环、她自己定节奏的循环都记在这里。
//!
//! 它是真相，不是派生的：和会话列表的索引、用量汇总不一样，坏了不删不重建（`open.rs`）。写盘照 `synchronous = FULL`，断电
//! 不丢已经答应了的改动。
//!
//! - 编号 `t<n>`，递增，删了的号不再用（`counter` 表）。
//! - 什么时候、跑在哪、谁建的照 JSON 存成一格：形状在 [`When`]、[`Target`]、[`Creator`]，加种类不改表。
//! - 到点怎么算、怎么交进会话不在这里（端点的 `schedules`）：这里只存取。

mod open;
mod row;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

use crate::root::DataRoot;
use crate::sqlite::DbError;

pub use open::Opened;

/// 一个任务。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// 编号 `t<n>`。
    pub id: String,
    /// 什么时候建的。
    pub created: Timestamp,
    /// 谁建的。
    pub by: Creator,
    /// 什么时候跑。
    pub when: When,
    /// 交进会话的那句话，原样存（循环的标记组装时包，不在这里）。
    pub prompt: String,
    /// 跑在哪。
    pub target: Target,
    /// 是不是循环、哪一种（施工 J-2 的 `/loop`、`schedule_wakeup`）；定时任务没有。
    pub looping: Option<Loop>,
    /// 停着：不到点。
    pub paused: bool,
    /// 下一次的时刻；停着的、她自己定节奏的循环这一轮还没定的没有。
    pub next: Option<Timestamp>,
    /// 上一次。
    pub last: Option<Last>,
}

/// 谁建的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Creator {
    /// 人：命令行、协议。
    Person,
    /// 她：在这个会话里用工具建的（施工 J-2）。
    Model {
        /// 建在哪个会话里。
        session: SessionId,
    },
    /// 包：扩展经协议建的（施工 J-4）。
    Package {
        /// 包的编号。
        package: String,
    },
}

/// 什么时候跑。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum When {
    /// cron，照 `zone` 那个时区读。
    Cron {
        /// 标准五段的写法。
        cron: String,
        /// 假的跑一次就删。
        recurring: bool,
        /// 照哪个时区读（施工 J-3 上）：IANA 的名字，例如 `Asia/Tokyo`。建时记下（人交的，不交的照这台机器那一刻的）；以前建的
        /// 没有，照本机的。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
    },
    /// 某个时刻一次。
    At {
        /// 那个时刻。
        at: Timestamp,
    },
    /// 每隔一阵一次。
    Every {
        /// 隔多少秒。
        every: u64,
    },
}

/// 跑在哪。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// 一个已有的会话。
    Session {
        /// 会话编号。
        session: SessionId,
    },
    /// 每次新开一个会话：人格、预设、工作目录建的时候就定下来。
    New {
        /// 人格的编号；没有的是无人格。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persona: Option<String>,
        /// 预设的编号。
        preset: String,
        /// 工作目录。
        cwd: String,
    },
    /// 建它的扩展自己（施工 J-4）：到点不进会话，推 `schedule.fired` 给那个包的进程。只有包建的有。
    Package {
        /// 核心停着时错过了几次，下一次推的时候一起报、报完清零（错过的照旧不补）。
        #[serde(default, skip_serializing_if = "is_zero")]
        missed: u32,
    },
}

/// 没错过：不写进表。
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde 的 skip_serializing_if 只交引用"
)]
fn is_zero(missed: &u32) -> bool {
    *missed == 0
}

/// 循环的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Loop {
    /// 固定间隔。
    Fixed,
    /// 她每轮结束自己定下一次。
    Dynamic,
}

/// 上一次跑的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Last {
    /// 什么时刻。
    pub at: Timestamp,
    /// `ran`、`queued`、`skipped:<原因>`。
    pub result: String,
    /// 跑在哪个会话：每次新开的是开出来的那个。
    pub session: Option<SessionId>,
}

/// 新建一个任务要的：编号、建的时刻表来给。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSchedule {
    /// 谁建的。
    pub by: Creator,
    /// 什么时候跑。
    pub when: When,
    /// 交进会话的那句话。
    pub prompt: String,
    /// 跑在哪。
    pub target: Target,
    /// 循环的种类；定时任务没有。
    pub looping: Option<Loop>,
    /// 第一次的时刻（照 `when` 算好的）。
    pub next: Option<Timestamp>,
}

/// 读不懂的一行（施工 AE-4）：某一格被改坏了、换了这一版不认的写法。[`Schedules::list`]、[`Schedules::due`] 跳过它，
/// 别的照常到点；[`Schedules::unreadable`] 交出来，调的一方照它记运行日志、`check` 报。人照编号删得掉。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// 编号；这一格也读不出的是 `?`。
    pub id: String,
    /// 为什么（英文一句：`bad when` 这样，说是哪一格）。
    pub why: String,
}

impl From<Unreadable> for DbError {
    fn from(one: Unreadable) -> DbError {
        DbError::Bad(format!("schedule {}: {}", one.id, one.why))
    }
}

/// 唤醒表：一个连接一直开着（`crate::sqlite` 的规矩），一次一个人用。
pub struct Schedules {
    db: Mutex<Connection>,
    path: PathBuf,
}

/// 唤醒表在哪。
pub fn path(root: &DataRoot) -> PathBuf {
    root.path().join("state").join("schedules.db")
}

impl Schedules {
    /// 打开唤醒表，没有就建；交回开着的表（打不开的没有）和是什么情形，调的一方照它记日志。
    pub fn open(root: &DataRoot) -> (Option<Schedules>, Opened) {
        let path = path(root);
        let (db, opened) = open::open(&path);
        (
            db.map(|db| Schedules {
                db: Mutex::new(db),
                path,
            }),
            opened,
        )
    }

    /// 库文件在哪。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 建一个：编号照计数器往后排，交回建好的一行。
    ///
    /// # Errors
    ///
    /// 写不了库。
    pub fn add(&self, new: NewSchedule, now: Timestamp) -> Result<Schedule, DbError> {
        let mut db = self.lock();
        let tx = db.transaction()?;
        tx.execute("UPDATE counter SET next = next + 1", [])?;
        let n: i64 = tx.query_row("SELECT next - 1 FROM counter", [], |row| row.get(0))?;
        let schedule = Schedule {
            id: format!("t{n}"),
            created: now,
            by: new.by,
            when: new.when,
            prompt: new.prompt,
            target: new.target,
            looping: new.looping,
            paused: false,
            next: new.next,
            last: None,
        };
        row::insert(&tx, &schedule)?;
        tx.commit()?;
        Ok(schedule)
    }

    /// 编号是 `id` 的那一个。
    ///
    /// # Errors
    ///
    /// 读不了库、一行读不懂。
    pub fn get(&self, id: &str) -> Result<Option<Schedule>, DbError> {
        let db = self.lock();
        Ok(db
            .query_row(row::SELECT_ONE, params![id], row::read)
            .optional()?
            .transpose()?)
    }

    /// 全部，照下一次的时刻排，停着的、没有下一次的排在后面，同一时刻照编号。读不懂的行跳过（施工 AE-4，[`Unreadable`]）。
    ///
    /// # Errors
    ///
    /// 读不了库。
    pub fn list(&self) -> Result<Vec<Schedule>, DbError> {
        Ok(self.read_all()?.0)
    }

    /// 读不懂的那几行（施工 AE-4），照 [`Schedules::list`] 的先后。
    ///
    /// # Errors
    ///
    /// 读不了库。
    pub fn unreadable(&self) -> Result<Vec<Unreadable>, DbError> {
        Ok(self.read_all()?.1)
    }

    /// 全部读一遍：读得懂的、读不懂的分开交。
    fn read_all(&self) -> Result<(Vec<Schedule>, Vec<Unreadable>), DbError> {
        let db = self.lock();
        let mut statement = db.prepare(row::SELECT_ALL)?;
        let rows = statement.query_map([], row::read)?;
        split(rows)
    }

    /// 换掉编号相同的那一行（停、续、记上一次、改下一次都是它）；没有的交回假。
    ///
    /// # Errors
    ///
    /// 写不了库。
    pub fn put(&self, schedule: &Schedule) -> Result<bool, DbError> {
        let db = self.lock();
        row::update(&db, schedule)
    }

    /// 只记上一次（到点交过以后）：`stop` 的同时停掉、清掉下一次。别的几格不动：她在那一轮里改了的下一次（施工 J-2）照她的。
    /// 没有的交回假。
    ///
    /// # Errors
    ///
    /// 写不了库。
    pub fn record(&self, id: &str, last: &Last, stop: bool) -> Result<bool, DbError> {
        let db = self.lock();
        row::record(&db, id, last, stop)
    }

    /// 删掉编号是 `id` 的那一行；没有的交回假。
    ///
    /// # Errors
    ///
    /// 写不了库。
    pub fn remove(&self, id: &str) -> Result<bool, DbError> {
        let db = self.lock();
        Ok(db.execute("DELETE FROM schedules WHERE id = ?1", params![id])? > 0)
    }

    /// 最早的下一次（停着的不算）；一个都没有的没有。
    ///
    /// # Errors
    ///
    /// 读不了库。
    pub fn earliest(&self) -> Result<Option<Timestamp>, DbError> {
        let db = self.lock();
        let at: Option<i64> = db.query_row(
            "SELECT min(next_at) FROM schedules WHERE paused = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(at.and_then(Timestamp::from_unix_millis))
    }

    /// 到点了的（下一次不晚于 `now`、没停），照时刻、编号排。读不懂的行跳过（施工 AE-4）：原来一行坏了整份交错，全机的定时
    /// 都不再响。
    ///
    /// # Errors
    ///
    /// 读不了库。
    pub fn due(&self, now: Timestamp) -> Result<Vec<Schedule>, DbError> {
        let db = self.lock();
        let mut statement = db.prepare(row::SELECT_DUE)?;
        let rows = statement.query_map(params![now.unix_millis()], row::read)?;
        Ok(split(rows)?.0)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // 拿着锁的那一次出错只会丢那一次的事务，连接照样能用。
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 读出来的行分成读得懂的、读不懂的；SQLite 自己出错的整份交错。
fn split(
    rows: impl Iterator<Item = rusqlite::Result<Result<Schedule, Unreadable>>>,
) -> Result<(Vec<Schedule>, Vec<Unreadable>), DbError> {
    let (mut kept, mut skipped) = (Vec::new(), Vec::new());
    for row in rows {
        match row? {
            Ok(schedule) => kept.push(schedule),
            Err(one) => skipped.push(one),
        }
    }
    Ok((kept, skipped))
}
