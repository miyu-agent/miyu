//! 检索库里的几样杂项（施工 W-15）：`meta(key, value)`，现在只有知识库上次建好的时刻 `indexed_at`（`knowledge/catch_up.rs`）。
//! 同指纹表：不换版本，开库时补上；删掉重建的库跟着没了，下次建好再记。

use rusqlite::{OptionalExtension, params};

use super::{DbError, RecallIndex};

/// 以前的库（同一个版本号）开的时候补上这张表：照原样建，已经有的不动（同指纹表）。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) WITHOUT ROWID";

impl RecallIndex {
    /// 记下 `key` 是 `value`，原来有的换掉。库用不了的不写。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.write(|tx| {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
    }

    /// `key` 记着的值。没记过的、库用不了的是空的。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn meta(&self, key: &str) -> Result<Option<String>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(None);
        };
        Ok(db
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()?)
    }
}
