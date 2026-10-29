//! 用量汇总（施工 8-15，`docs/designs/07-存储.md` 第六节、S3，`docs/blueprint/models.md`「怎么走」第九条第 4 到 6 条）：
//! `state/usage.db`，一个核心一份 SQLite，一个连接一直开着（和会话列表的索引一样，[`crate::sqlite`]）。它是派生的：真相是
//! 会话的日志（`model.called`）和账号日志（`journal.jsonl`）里的 `usage.purged`、`usage.oneshot`，表随时可以删掉照它们重建。
//!
//! 表 `spent` 一行是一次请求，或者删掉的会话一个小时里一个（供应商、模型、用途）的合计（[`spent`]）；主键（`source`、`seq`），
//! 重复写不出两行。表 `marks` 记每一份日志读到了哪里：会话的照 [`crate::log::Mark`]，账号日志照字节；连同会话的属主、场所、父会话，
//! 删掉的会话标 `purged`。
//!
//! - 写（[`UsageIndex::advance`]）：会话每落一批，actor 在阻塞线程里写这一批的行；记到的位置正好是这一批之前的才往前挪，
//!   中间哪一批没写上的停在原处，等补（`catch_up.rs`）。
//! - 补（[`UsageIndex::catch_up`]）：查询之前，账号日志、管理员的会话、回收处里的会话，记到的照 [`crate::log::read_marked`]
//!   读多出来的一截，没记过的、对不上的整份读。空的表照这样补满，就是重建。
//! - 一次性调用（[`UsageIndex::one_shot`]）：先往调的那个账号的 `journal.jsonl` 追加一条 `usage.oneshot`，再写一行。
//! - 删掉的会话（[`purged`]）：回收处清它之前写进账号日志的 `usage.purged`，补的时候换掉它的单次行。
//! - 查（[`UsageIndex::query`]）：照分组加起来，金额照币种各加各的（`query.rs`）；换算（施工 UC-1，[`UsageIndex::convert`]）
//!   照每一笔那天的汇率换成一个币种（`convert.rs`），汇率在 `state/rates.db`（[`rates`]）。

mod catch_up;
mod convert;
pub mod purged;
mod query;
pub mod rates;
pub mod spent;

pub use catch_up::Problem;
pub use convert::Converted;
pub use query::{Group, Query, Total};
pub use spent::{OneShotCall, Who};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use miyu_kernel::time::Timestamp;

use rusqlite::Connection;

use crate::root::DataRoot;
use crate::sqlite::{self, DbError, Opened, connect, remove};

/// 用量汇总读写出错（[`crate::sqlite::DbError`]）。
pub type UsageError = DbError;

/// 库的文件名，在数据根的 `state/` 下。
pub const FILE: &str = "usage.db";

/// 日志里一次性调用那一条的种类。
pub const ONESHOT: &str = "usage.oneshot";

/// 日志里删掉的会话那一条的种类。
pub const PURGED: &str = "usage.purged";

/// 表的结构的版本，记在 SQLite 的 `user_version` 里：结构一变就加一，对不上的删掉重建。施工 SU-6 上去掉 `owner`，是 2。
const VERSION: i64 = 2;

/// 建表。时刻存成毫秒；金额存成双精度，币种另一格。
///
/// - `spent.source`：会话的请求是会话编号，一次性调用是 `journal`，删掉的会话的合计是 `purged:<会话编号>`；
///   `seq` 是 `model.called` 的序号、日志里那一条的序号、合计的第几格。
/// - `marks.source`：会话编号，或者 `journal`；会话的 `segment`、`bytes`、`next` 是 [`crate::log::Mark`]，日志的
///   `bytes` 是读到第几个字节，`segment` 是 0。
const SCHEMA: &str = "CREATE TABLE spent (
    source TEXT NOT NULL,
    seq INTEGER NOT NULL,
    at INTEGER NOT NULL,
    venue TEXT,
    session TEXT,
    parent TEXT,
    purpose TEXT,
    summary INTEGER NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    requests INTEGER NOT NULL,
    unpriced INTEGER NOT NULL,
    uncached INTEGER NOT NULL,
    cache_read INTEGER NOT NULL,
    cache_write INTEGER NOT NULL,
    output INTEGER NOT NULL,
    amount REAL,
    currency TEXT,
    PRIMARY KEY (source, seq)
) WITHOUT ROWID;
CREATE INDEX spent_session ON spent (session);
CREATE TABLE marks (
    source TEXT PRIMARY KEY NOT NULL,
    venue TEXT,
    parent TEXT,
    segment INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    next INTEGER NOT NULL,
    purged INTEGER NOT NULL
) WITHOUT ROWID";

/// 用量汇总：一个核心一份。
#[derive(Debug)]
pub struct UsageIndex {
    /// 数据根：日志、会话的目录照它找。
    root: DataRoot,
    /// 库文件在哪。
    path: PathBuf,
    /// 开着的连接：用不了的（删了重建也打不开）是空的，这时查出来没有一行、写什么都不写。
    db: Mutex<Option<Connection>>,
    /// 汇率（施工 UC-1）：换算照它找，取汇率的端口由核心起来时装上（[`UsageIndex::rates`]）。
    rates: Arc<rates::RateBook>,
}

impl UsageIndex {
    /// 打开数据根 `root` 的用量汇总（`state/usage.db`），没有就建。读不了、坏了、版本不对的删掉建一份空的：空的照日志补满
    /// （[`UsageIndex::catch_up`]）。
    pub fn open(root: &DataRoot) -> (UsageIndex, Opened) {
        let path = root.state().join(FILE);
        let (db, opened) = sqlite::open(&path, SCHEMA, VERSION);
        let rates = Arc::new(rates::RateBook::open(&root.state().join(rates::FILE)));
        let index = UsageIndex {
            root: root.clone(),
            path,
            db: Mutex::new(db),
            rates,
        };
        (index, opened)
    }

    /// 删掉重建：用着用着读出坏了的，关上连接，连同 `-wal`、`-shm` 删掉，建一份空的。
    ///
    /// # Errors
    ///
    /// 删不掉、建不成：这之后用不了。
    pub fn reset(&self) -> Result<(), UsageError> {
        let mut db = self.lock();
        drop(db.take());
        remove(&self.path)?;
        *db = Some(connect(&self.path, SCHEMA, VERSION)?.0);
        Ok(())
    }

    /// 找汇率的（施工 UC-1）：核心起来时给它装上取汇率的端口（[`rates::RateBook::install`]）。
    pub fn rates(&self) -> &rates::RateBook {
        &self.rates
    }

    /// 把 `daily`（按 UTC 日子、币种分开的金额）换成 `currency`（施工 UC-1）：照 `now` 找汇率，缺的当场去取、最多等 `wait`
    /// （写 0 的不等：只用缓存，缺的在后面取）。一笔都没换成的没有。
    pub fn convert(
        &self,
        daily: &[(rates::Day, String, f64)],
        currency: &str,
        now: Timestamp,
        wait: Duration,
    ) -> Option<Converted> {
        self.convert_all(&[daily], currency, now, wait)
            .pop()
            .flatten()
    }

    /// 同 [`UsageIndex::convert`]，几组一起：汇率只找一次（缺的并成一次去取、只等一回），照给的先后交回。
    pub fn convert_all(
        &self,
        groups: &[&[(rates::Day, String, f64)]],
        currency: &str,
        now: Timestamp,
        wait: Duration,
    ) -> Vec<Option<Converted>> {
        let wanted: BTreeSet<(rates::Day, String)> = groups
            .iter()
            .flat_map(|daily| daily.iter())
            .flat_map(|(day, from, _)| [(*day, from.clone()), (*day, currency.to_string())])
            .collect();
        let found = match wanted.is_empty() {
            true => BTreeMap::new(),
            false => self.rates.rates(&wanted, now.unix_millis(), wait),
        };
        groups
            .iter()
            .map(|daily| convert::convert(daily, currency, &found))
            .collect()
    }

    /// 库文件在哪。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 拿着连接。拿着锁的线程 panic 了也照样用：连接本身没坏，坏了的下次读出来再重建。
    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 日志 `journal.jsonl` 在 `marks`、一次性调用在 `spent` 里的 `source`。
const JOURNAL: &str = "journal";

/// 删掉的会话 `session` 的合计在 `spent` 里的 `source`。
fn purged_source(session: &str) -> String {
    format!("purged:{session}")
}
