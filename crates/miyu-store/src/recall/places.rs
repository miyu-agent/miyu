//! 回合库里哪个会话在哪个场所（施工 R-13 下，`docs/blueprint/recall.md`「库的结构」，`memory.md` 第一条第 1 款）：
//! `places(source, venue)`，场所会话接上回合索引时记一次，本机的会话不记。搜以前的对话照它挑：一个场所里只搜得到会话在这个
//! 场所的（`memory.md` 第九条）。删会话时跟着那个来源拿掉（`recall.rs` 的 `forget`），库删了照会话日志补。

use std::collections::BTreeMap;

use rusqlite::params;

use super::{DbError, RecallIndex};

/// 以前的库（同一个版本号）开的时候补上这张表：照原样建，已经有的不动（同向量表）。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS places (
    source TEXT PRIMARY KEY NOT NULL,
    venue TEXT NOT NULL
) WITHOUT ROWID";

impl RecallIndex {
    /// 记下来源 `source`（一个会话）在场所 `venue`：记过的换成这一次的。库用不了的不写。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn place(&self, source: &str, venue: &str) -> Result<(), DbError> {
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO places (source, venue) VALUES (?1, ?2)",
                params![source, venue],
            )?;
            Ok(())
        })
    }

    /// 记着的每个来源在哪个场所；本机的会话不在里面。库用不了的是空的。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn places(&self) -> Result<BTreeMap<String, String>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(BTreeMap::new());
        };
        let mut select = db.prepare("SELECT source, venue FROM places")?;
        let rows = select.query_map(params![], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut places = BTreeMap::new();
        for row in rows {
            let (source, venue) = row?;
            places.insert(source, venue);
        }
        Ok(places)
    }
}
