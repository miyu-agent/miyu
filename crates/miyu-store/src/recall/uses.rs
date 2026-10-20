//! 回合库里的「用到」（施工 R-4 下，`docs/blueprint/recall.md`「库的结构」，`memory.md` 第八条）：`uses(key, memory, at)`，
//! 会话日志里一条事件带上一条记忆写一行，键是 `会话/事件的序号`，同一行再写不重复。照会话日志派生（`miyu_recall::uses`），
//! 删会话时跟着那个来源拿掉（`recall.rs` 的 `forget`），库删了照日志补。

use std::collections::BTreeMap;

use rusqlite::params;

use miyu_kernel::time::Timestamp;

use super::{DbError, RecallIndex};

/// 以前的库（同一个版本号）开的时候补上这张表：照原样建，已经有的不动（同向量表）。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS uses (
    key TEXT NOT NULL,
    memory TEXT NOT NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (key, memory)
) WITHOUT ROWID";

/// 一条记忆用到过几次、最近一次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Used {
    /// 几次：带上它的事件有几条。
    pub count: u64,
    /// 最近一次：那几条事件里最晚的时刻。
    pub last: Timestamp,
}

impl RecallIndex {
    /// 每一条用到过的记忆（键是它的编号，`m12` 这种）用到几次、最近一次；没用到过的不在里面。库用不了的是空的。
    ///
    /// # Errors
    ///
    /// 读不了；记着的时刻、次数不对。
    pub fn uses(&self) -> Result<BTreeMap<String, Used>, DbError> {
        let db = self.reading();
        let Some(db) = db.as_ref() else {
            return Ok(BTreeMap::new());
        };
        let mut select =
            db.prepare("SELECT memory, COUNT(*), MAX(at) FROM uses GROUP BY memory")?;
        let rows = select.query_map(params![], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        let mut uses = BTreeMap::new();
        for row in rows {
            let (memory, count, last) = row?;
            let count =
                u64::try_from(count).map_err(|_| DbError::Bad(format!("use count {count}")))?;
            let last = Timestamp::from_unix_millis(last)
                .ok_or_else(|| DbError::Bad(format!("use at {last}")))?;
            uses.insert(memory, Used { count, last });
        }
        Ok(uses)
    }
}
