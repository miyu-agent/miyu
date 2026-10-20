//! 检索库里每个来源的指纹（施工 K-1 上，`docs/blueprint/recall.md`「库的结构」，`knowledge.md`「怎么走」第三条）：
//! `stamps(source, stamp)`。知识库的一个文档是一个来源，块的键是 `<来源>#<第几块>`；文档变了（指纹不一样）整份换掉，在一个
//! 事务里拿掉原来的块、放进新的、记下新的指纹。指纹的写法由调的一方定（知识库是大小加改的时刻），库不解读。

use std::collections::BTreeMap;

use rusqlite::{Transaction, params};

use miyu_kernel::time::Timestamp;

use super::{DbError, RecallIndex, put};

/// 以前的库（同一个版本号）开的时候补上这张表：照原样建，已经有的不动（同向量表）。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS stamps (
    source TEXT PRIMARY KEY NOT NULL,
    stamp TEXT NOT NULL
) WITHOUT ROWID";

impl RecallIndex {
    /// 整份换掉来源 `source`：拿掉它原来的块（键以 `<source>#` 开头的），照先后放进 `chunks`（键是 `<source>#<第几块>`，块从 0
    /// 起，时刻都是 `at`），记下指纹 `stamp`，在一个事务里。库用不了的不写。
    ///
    /// # Errors
    ///
    /// 写不进：这一个来源照旧，下次再换。
    pub fn replace(
        &self,
        source: &str,
        chunks: &[String],
        at: Timestamp,
        stamp: &str,
    ) -> Result<(), DbError> {
        self.write(|tx| {
            clear(tx, source)?;
            for (n, text) in chunks.iter().enumerate() {
                put(tx, &format!("{source}#{n}"), text, at)?;
            }
            tx.execute(
                "INSERT INTO stamps (source, stamp) VALUES (?1, ?2)",
                params![source, stamp],
            )?;
            Ok(())
        })
    }

    /// 只记下来源 `source` 的指纹 `stamp`，块照留（施工 W-15 再补：跳过的文档也记，原来的块不动）。库用不了的不写。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn stamp_only(&self, source: &str, stamp: &str) -> Result<(), DbError> {
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO stamps (source, stamp) VALUES (?1, ?2)",
                params![source, stamp],
            )?;
            Ok(())
        })
    }

    /// 拿掉来源 `source` 的全部块和它的指纹，在一个事务里（文档没了）。没有的不要紧。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn drop_source(&self, source: &str) -> Result<(), DbError> {
        self.write(|tx| clear(tx, source))
    }

    /// 记着的每个来源的指纹。库用不了的是空的。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn stamps(&self) -> Result<BTreeMap<String, String>, DbError> {
        let db = self.reading();
        let Some(db) = db.as_ref() else {
            return Ok(BTreeMap::new());
        };
        let mut select = db.prepare("SELECT source, stamp FROM stamps")?;
        let rows = select.query_map(params![], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut stamps = BTreeMap::new();
        for row in rows {
            let (source, stamp) = row?;
            stamps.insert(source, stamp);
        }
        Ok(stamps)
    }
}

/// 拿掉来源 `source` 的块（连同词、向量）和指纹。照 `#` 分：一个来源是另一个的前缀（`a.md`、`a.md.bak`）也不碍事。
fn clear(tx: &Transaction<'_>, source: &str) -> Result<(), DbError> {
    // 照键的范围删（`#` 的下一个字符是 `$`）：走 `key` 上的索引。原来照 `substr` 比，每个来源都扫一遍整张表，三千篇的库头一次补
    // 要 5 秒上下，改了以后 0.6 秒上下（施工 K-4 量的）。
    let (from, to) = (format!("{source}#"), format!("{source}$"));
    tx.execute(
        "DELETE FROM terms WHERE rowid IN (SELECT id FROM items WHERE key >= ?1 AND key < ?2)",
        params![from, to],
    )?;
    tx.execute(
        "DELETE FROM items WHERE key >= ?1 AND key < ?2",
        params![from, to],
    )?;
    tx.execute(
        "DELETE FROM vectors WHERE key >= ?1 AND key < ?2",
        params![from, to],
    )?;
    tx.execute("DELETE FROM stamps WHERE source = ?1", [source])?;
    Ok(())
}
