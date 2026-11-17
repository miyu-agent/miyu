//! 人生的两样小账（施工 LF-14，设计 `70-推演人生.md` 第三节，`docs/blueprint/schedules.md` 第一节）：各一张小表，同 `doing`
//! 照「只加表不升版本」开库时补上。
//!
//! - `rounds`：人生那一行推演到第几轮，照任务编号存。每次到点交进会话加一（端点的 `schedules/run.rs`）；删那一行跟着删。
//!   只记不显示：给头的数据里有，界面上不写（项目主人 2026-11-17 定：写出来不像真的在过日子）。
//! - `life_unseen`：她自己结束了、主人还没看过的人生会话，照会话编号存：是哪个人格的、什么时候结束的。有头打开那个会话、
//!   说一声看过了、删了那个会话就清掉（端点的 `life/unseen.rs`）。

use rusqlite::{Connection, params};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

use crate::sqlite::DbError;

/// 开库时补上这两张表：照原样建，已经有的不动。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS rounds (
    id TEXT PRIMARY KEY NOT NULL,
    count INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS life_unseen (
    session TEXT PRIMARY KEY NOT NULL,
    persona TEXT,
    at INTEGER NOT NULL
) WITHOUT ROWID";

/// 她自己结束了、还没人看过的一段人生。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unseen {
    /// 那个人生会话。
    pub session: SessionId,
    /// 它是哪个人格的；会话那时不带人格的没有。
    pub persona: Option<String>,
    /// 结束的时刻。
    pub at: Timestamp,
}

/// 任务 `id` 又推演了一轮：加一，交回加完是第几轮。
pub(super) fn add_round(db: &Connection, id: &str) -> Result<u64, DbError> {
    let count: i64 = db.query_row(
        "INSERT INTO rounds (id, count) VALUES (?1, 1) \
         ON CONFLICT (id) DO UPDATE SET count = count + 1 RETURNING count",
        params![id],
        |row| row.get(0),
    )?;
    Ok(u64::try_from(count).unwrap_or(0))
}

/// 任务 `id` 推演到第几轮；一轮都没有的是 0。
pub(super) fn rounds(db: &Connection, id: &str) -> Result<u64, DbError> {
    let mut found = db.prepare("SELECT count FROM rounds WHERE id = ?1")?;
    let mut rows = found.query(params![id])?;
    match rows.next()? {
        Some(row) => Ok(u64::try_from(row.get::<_, i64>(0)?).unwrap_or(0)),
        None => Ok(0),
    }
}

/// 任务 `id` 没了：它的轮数跟着删。
pub(super) fn forget_rounds(db: &Connection, id: &str) -> Result<(), DbError> {
    db.execute("DELETE FROM rounds WHERE id = ?1", params![id])?;
    Ok(())
}

/// 还没人看过的那几段，结束得早的在前。会话编号、时刻读不懂的那一行跳过。
pub(super) fn unseen(db: &Connection) -> Result<Vec<Unseen>, DbError> {
    let mut listed =
        db.prepare("SELECT session, persona, at FROM life_unseen ORDER BY at, session")?;
    let rows = listed.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut found = Vec::new();
    for row in rows {
        let (session, persona, at) = row?;
        if let (Ok(session), Some(at)) =
            (SessionId::parse(&session), Timestamp::from_unix_millis(at))
        {
            found.push(Unseen {
                session,
                persona,
                at,
            });
        }
    }
    Ok(found)
}

/// 记下一段还没人看过的：同一个会话再记一次换成新的。
pub(super) fn mark(db: &Connection, one: &Unseen) -> Result<(), DbError> {
    db.execute(
        "INSERT INTO life_unseen (session, persona, at) VALUES (?1, ?2, ?3) \
         ON CONFLICT (session) DO UPDATE SET persona = excluded.persona, at = excluded.at",
        params![one.session.as_str(), one.persona, one.at.unix_millis()],
    )?;
    Ok(())
}

/// 会话 `session` 的那一段看过了：清掉，原来记着的交回真。
pub(super) fn seen(db: &Connection, session: &SessionId) -> Result<bool, DbError> {
    let removed = db.execute(
        "DELETE FROM life_unseen WHERE session = ?1",
        params![session.as_str()],
    )?;
    Ok(removed > 0)
}

#[cfg(test)]
mod tests;
