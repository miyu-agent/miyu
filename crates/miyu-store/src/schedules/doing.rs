//! 人生那一行上她写的「现在在干嘛」（施工 LF-7，设计 `70-推演人生.md` 第三节第 8 条，`docs/blueprint/schedules.md` 第一节）：
//! 另一张小表 `doing`，一个任务至多一行，照任务编号存。
//!
//! - 为什么不是 `schedules` 多一列：「只加表、加索引的改动不升版本」，加列要升版本、写迁移；而且 `put` 整行写回，到点那一刻
//!   读出的旧行会盖掉她这一轮刚写的一句。单放一张表，只有 [`super::Schedules::set_doing`] 写它，`put`、`record` 碰不到。
//! - 以前建的库（同一个版本号）开的时候照 [`ADD`] 补上，已经有的不动（同回合库的向量表）。比这一版旧的程序看不见它：它删了
//!   任务留下的那一行编号以后不再用，读的时候照任务编号找，孤着的不碍事。

use rusqlite::{Connection, OptionalExtension, params};

use miyu_kernel::time::Timestamp;

use crate::sqlite::DbError;

/// 开库时补上这张表：照原样建，已经有的不动。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS doing (
    id TEXT PRIMARY KEY NOT NULL,
    text TEXT NOT NULL,
    at INTEGER NOT NULL,
    until INTEGER NOT NULL
) WITHOUT ROWID";

/// 她写的「现在在干嘛」：那一句、写下的时刻、那时定的下次醒的时刻。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doing {
    /// 她写的那一句：存之前由调的一方清理、截短（端点的 `schedules/doing.rs`），这里原样存。
    pub text: String,
    /// 写下的时刻。
    pub at: Timestamp,
    /// 写下时定的下次醒的时刻。之后照默认间隔醒的那几次不改它。
    pub until: Timestamp,
}

/// 任务 `id` 的那一行；没有的、时刻读不懂的没有（时刻读不懂的当她没写过）。
pub(super) fn read(db: &Connection, id: &str) -> Result<Option<Doing>, DbError> {
    let found: Option<(String, i64, i64)> = db
        .query_row(
            "SELECT text, at, until FROM doing WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    Ok(found.and_then(|(text, at, until)| {
        Some(Doing {
            text,
            at: Timestamp::from_unix_millis(at)?,
            until: Timestamp::from_unix_millis(until)?,
        })
    }))
}

/// 任务 `id` 的那一行换成 `doing`；没有的删掉。
pub(super) fn write(db: &Connection, id: &str, doing: Option<&Doing>) -> Result<(), DbError> {
    match doing {
        Some(doing) => db.execute(
            "INSERT INTO doing (id, text, at, until) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT (id) DO UPDATE SET text = excluded.text, at = excluded.at, until = excluded.until",
            params![
                id,
                doing.text,
                doing.at.unix_millis(),
                doing.until.unix_millis()
            ],
        )?,
        None => db.execute("DELETE FROM doing WHERE id = ?1", params![id])?,
    };
    Ok(())
}

#[cfg(test)]
mod tests;
