//! 汇率的缓存（施工 UC-1）：`state/rates.db`，一行一个（UTC 日子，币种）：1 美元换多少、哪个源、什么时候取的、定没定。
//! 取不到的币种（源说不认识）也记一行，汇率是空的，同暂定的一样过期重取，不一直问。

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};

use super::book::{Fetched, Source};
use super::{Day, day_of};
use crate::sqlite::{self, idle};

/// 库的文件名，在数据根的 `state/` 下。
pub const FILE: &str = "rates.db";

/// 表的结构的版本：一变就加一，对不上的删掉重建。
const VERSION: i64 = 1;

/// 暂定的汇率多久以后重取。
const STALE_MS: i64 = 6 * 3_600_000;

const SCHEMA: &str = "CREATE TABLE rates (
    day INTEGER NOT NULL,
    currency TEXT NOT NULL,
    rate REAL,
    source TEXT NOT NULL,
    fetched INTEGER NOT NULL,
    settled INTEGER NOT NULL,
    PRIMARY KEY (day, currency)
) WITHOUT ROWID";

/// 缓存里的一行。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cached {
    /// 1 美元换多少；源不认识这个币种的是空的。
    pub(crate) rate: Option<f64>,
    /// 哪个源。
    pub(crate) source: Source,
    /// 什么时候取的，毫秒。
    pub(crate) fetched: i64,
    /// 定了：不再取。
    pub(crate) settled: bool,
}

impl Cached {
    /// 到 `now`（毫秒）还用不用重取：定了的不用；暂定的取了不到 6 小时、而且还是取的那个 UTC 日子，不用。
    pub(crate) fn fresh(&self, now: i64) -> bool {
        self.settled || (now - self.fetched < STALE_MS && day_of(self.fetched) == day_of(now))
    }
}

/// 汇率的缓存：一个核心一份，一个连接一直开着。用不了的（删了重建也打不开）是空的：读不到、写不进，每次都去取。
#[derive(Debug)]
pub(crate) struct Cache {
    /// 登记给闲时合回（施工 V-28）。
    db: Arc<Mutex<Option<Connection>>>,
}

impl Cache {
    /// 打开 `path`，没有就建；坏了、版本不对的删掉重建。
    pub(crate) fn open(path: &Path) -> Cache {
        let (db, _) = sqlite::open(path, SCHEMA, VERSION);
        Cache {
            db: idle::keep("rates", Mutex::new(db)),
        }
    }

    /// （`day`，`currency`）那一行；没有的、读不了的没有。
    pub(crate) fn get(&self, day: Day, currency: &str) -> Option<Cached> {
        let db = self.lock();
        let db = db.as_ref()?;
        db.query_row(
            "SELECT rate, source, fetched, settled FROM rates WHERE day = ?1 AND currency = ?2",
            params![day, currency],
            |row| {
                let source: String = row.get(1)?;
                Ok(Cached {
                    rate: row.get(0)?,
                    source: Source::parse(&source),
                    fetched: row.get(2)?,
                    settled: row.get::<_, i64>(3)? != 0,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    /// 记下取到的，取的时刻是 `fetched`（毫秒）：同一个（日子，币种）换掉旧的。交回写进去了没有；写不进的不要紧，下次再取。
    pub(crate) fn put(&self, rows: &[Fetched], fetched: i64) -> bool {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return false;
        };
        let Ok(tx) = db.transaction() else {
            return false;
        };
        for row in rows {
            let written = tx.execute(
                "INSERT OR REPLACE INTO rates (day, currency, rate, source, fetched, settled) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    row.day,
                    row.currency,
                    row.rate,
                    row.source.as_str(),
                    fetched,
                    i64::from(row.settled)
                ],
            );
            if written.is_err() {
                return false;
            }
        }
        tx.commit().is_ok()
    }

    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
