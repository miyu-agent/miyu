//! 核心停下时的收尾（`internal/blueprint/core.md`「停下」，施工 CS-9 从 `lib.rs` 挪出来）：停会话、等记忆的读写做完、关库。
//! 都在放数据根的锁以前做：交回以后这一份核心不再碰数据根。

use std::sync::Arc;

use crate::Core;

impl Core {
    /// 有计划地停下全部在跑的会话：核心收到停的信号时。跑到一半的回合记成「重启了」，下次载入接着干。正在造的会话不等它手里
    /// 的表的锁（施工 CS-9，`sessions/stopping.rs`）。停完了等记忆在后台的读写做完（[`Core::finish_writes`]，施工 TMP-1 补3）。
    pub async fn stop_sessions(&self) {
        self.sessions.stop_all().await;
        self.finish_writes().await;
    }

    /// 等记忆在后台还没做完的（施工 TMP-1 补3，`Memory::finish`）：回合开始问记忆、会话没等完的读，回合索引的后台，补齐旧会话的
    /// 线程。核心停下、放锁以前等：交回以后它们不再碰数据根。在阻塞线程里等。
    pub async fn finish_writes(&self) {
        let memory = Arc::clone(&self.memory);
        if tokio::task::spawn_blocking(move || memory.finish())
            .await
            .is_err()
        {
            tracing::error!(target: "miyu::endpoint", "memory writes wait panicked");
        }
    }

    /// 关上数据根里开着的库（施工 CS-9，`miyu_store::sqlite::idle::close_under`）：会话列表的索引、用量汇总、汇率、唤醒表、
    /// 检索库，写的、读的连接都关。关的时候不合 `-wal`（盘慢时一个库要等两次同步），下次开库合回。核心停下、放锁以前、收尾
    /// 的最后调：之后再来的读写照「库用不了」，不碰数据根。关不上的一种库记一行 `WARN database not closed db=… error=…`。
    /// 在阻塞线程里关：正在写的连接等它写完。
    pub async fn close_databases(&self) {
        let root = self.root.path().to_path_buf();
        let closed =
            tokio::task::spawn_blocking(move || miyu_store::sqlite::idle::close_under(&root)).await;
        match closed {
            Ok(closed) => {
                for (db, error) in &closed.failed {
                    tracing::warn!(target: "miyu::endpoint", db, error = %error, "database not closed");
                }
            }
            Err(_) => tracing::error!(target: "miyu::endpoint", "closing databases panicked"),
        }
    }
}
