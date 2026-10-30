//! 检索库的两个连接（施工 V-14）：写的一个、读的一个，各拿各的锁。
//!
//! 原来一个连接一把锁，读写排着队：会话每落一批，后台把它写进回合库，提交时 WAL 攒够了页就顺手做检查点、同步，拿着锁十几到
//! 二十几毫秒（大会话一句接一句说，七十来句碰上一次）；回合开始找记忆正好撞上的，那一轮的请求跟着晚这么久。WAL 里读不等写：
//! 读的连接看到的是最近一次提交，写到一半、做检查点都不挡它。
//!
//! 读的连接不建表、不体检（`quick_check` 在写的连接开库时做过），设成只读；开不了的这个库这一回用不了。写的连接登记给闲时
//! 合回（施工 V-28，[`crate::sqlite::idle`]）：合的时候拿它的锁，读的不等。两个连接都一直开着：
//! 同一个进程里在 SQLite 外面开了又关同一个库文件会丢掉文件锁（`07-存储.md` 第六节），SQLite 自己开的几个连接不碰这一条。

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, Transaction};

use super::{RecallIndex, SCHEMA, VERSION, add};
use crate::sqlite::{self, DbError, Opened, connect, idle, remove};

impl RecallIndex {
    /// 打开 `path` 这一份库，没有就建（目录一起建，Unix 上 0700）。读不了、坏了、版本不对的，连同 SQLite 的 `-wal`、
    /// `-shm` 删掉，建一份空的，调的一方照真相补。删了重建也打不开的，这一回用不了（[`Opened::Unusable`]）。
    pub fn open(path: &Path) -> (RecallIndex, Opened) {
        let index = RecallIndex::absent(path);
        let opened = index.build();
        (index, opened)
    }

    /// 还没开的一份（施工 V-28：记忆库用到才建）：两个连接都没有，读出来什么都没有、写什么都不写，盘上什么都不建，等
    /// [`RecallIndex::build`]。
    pub(crate) fn absent(path: &Path) -> RecallIndex {
        RecallIndex {
            path: path.to_path_buf(),
            db: idle::keep("recall", Mutex::new(None)),
            reader: Mutex::new(None),
        }
    }

    /// 开库、开两个连接，照 [`RecallIndex::open`] 的规矩，交回是什么情形；已经开着的不动，交回 [`Opened::Kept`]。
    pub(crate) fn build(&self) -> Opened {
        let (mut db, mut reader) = (self.lock(), self.reading());
        if db.is_some() {
            return Opened::Kept;
        }
        let (fresh, opened) = sqlite::open(&self.path, SCHEMA, VERSION);
        // 以前的库（同一个版本、没有向量表、用到的表）补上这几张表；补不上的这一回用不了。
        match fresh.map(|fresh| both(&self.path, fresh)) {
            Some(Ok((fresh, read))) => {
                *db = Some(fresh);
                *reader = Some(read);
                opened
            }
            Some(Err(error)) => Opened::Unusable(error),
            None => opened,
        }
    }

    /// 删掉重建：用着用着读出坏了的，关上连接，连同 `-wal`、`-shm` 删掉，建一份空的。
    ///
    /// # Errors
    ///
    /// 删不掉、建不成：这之后用不了这个库。
    pub fn reset(&self) -> Result<(), DbError> {
        let (mut db, mut reader) = (self.lock(), self.reading());
        drop((db.take(), reader.take()));
        remove(&self.path)?;
        let (fresh, read) = both(&self.path, connect(&self.path, SCHEMA, VERSION)?.0)?;
        *db = Some(fresh);
        *reader = Some(read);
        Ok(())
    }

    /// 关上连接，连同 `-wal`、`-shm` 删掉，以后什么都不写（施工 K-7 上：删知识库）。还拿着这一份的（会话在后台补向量）照
    /// 「库用不了的不写」。
    ///
    /// # Errors
    ///
    /// 删不掉。
    pub fn discard(&self) -> Result<(), DbError> {
        let (mut db, mut reader) = (self.lock(), self.reading());
        drop((db.take(), reader.take()));
        remove(&self.path)
    }

    /// 在一个事务里写；库用不了的什么都不做。
    pub(super) fn write(
        &self,
        body: impl FnOnce(&Transaction<'_>) -> Result<(), DbError>,
    ) -> Result<(), DbError> {
        let mut db = self.lock();
        let Some(db) = db.as_mut() else {
            return Ok(());
        };
        let tx = db.transaction()?;
        body(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// 读的连接。别的线程拿着锁崩了，库还是好的，照常用。
    pub(super) fn reading(&self) -> MutexGuard<'_, Option<Connection>> {
        self.reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 写的连接。别的线程拿着锁崩了，库还是好的（写都在事务里），照常用。重建、关上时先拿它再拿读的，读写各自只拿一把。
    fn lock(&self) -> MutexGuard<'_, Option<Connection>> {
        self.db
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// 开好的写连接补上不换版本加的几张表，再开读的连接。
fn both(path: &Path, db: Connection) -> Result<(Connection, Connection), DbError> {
    add(&db)?;
    let reader = Connection::open(path)?;
    reader.pragma_update(None, "query_only", true)?;
    Ok((db, reader))
}

#[cfg(test)]
mod tests;
