//! 闲时把库的写前日志合回主库（施工 V-28，`docs/blueprint/store.md`「怎么走」的「库的写前日志」）：起一个线程，照
//! `miyu_store::sqlite::idle` 等到库有过提交、之后 [`QUIET`] 这么久再没有提交，把每个开着的库的 `-wal` 合回主库、截成 0 字节，
//! 再回去等。
//!
//! 照「库最后一次写过去一阵子」算，不照「最后一个请求过去一阵子」（`settle.rs`）：一轮任务跑着的时候多半没有请求，会话照样
//! 一批批写索引、用量、回合库；照请求等，任务跑完以前一直合不上，被杀时那一截全留在 `-wal` 里。没有写的时候它一直睡着，不定时
//! 醒。一个进程只起一个（测试里一个进程开好几个核心，库的登记也是一个进程一份）。

use std::sync::Once;
use std::time::Duration;

use miyu_store::sqlite::idle::{self, Checkpointed};

use crate::TARGET;

/// 库最后一次提交以后多久算闲下来了：和请求的 `settle::QUIET` 一样长。一轮任务里等模型的那几秒也算，那时合一次几毫秒。
pub const QUIET: Duration = Duration::from_secs(2);

/// 起合的线程；起过的不再起。起不来的记一行 `WARN wal checkpoints not started`：库照旧，`-wal` 照 SQLite 自己的检查点和
/// `journal_size_limit` 缩，好好退出时 SQLite 自己合回、删掉。
pub fn spawn() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("miyu-wal".to_string())
            .spawn(|| {
                loop {
                    idle::wait_quiet(QUIET);
                    report(&idle::checkpoint_all());
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(target: TARGET, error = %error, "wal checkpoints not started");
        }
    });
}

/// 合了一遍，记日志：合了的、没合完的一行 `DEBUG wal checkpointed done=… busy=…`（都没有的不记），合不了的一个一行
/// `WARN wal not checkpointed db=… error=…`。
fn report(done: &Checkpointed) {
    if done.done > 0 || done.busy > 0 {
        tracing::debug!(target: TARGET, done = done.done, busy = done.busy, "wal checkpointed");
    }
    for (db, error) in &done.failed {
        tracing::warn!(target: TARGET, db, error = %error, "wal not checkpointed");
    }
}
