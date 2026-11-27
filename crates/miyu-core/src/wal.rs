//! 闲时把库的写前日志合回主库（施工 V-28，`internal/blueprint/store.md`「怎么走」的「库的写前日志」）：起一个线程，照
//! `miyu_store::sqlite::idle` 等到库有过提交、之后 [`QUIET`] 这么久再没有提交，每个开着的连接（写的、只读的）放掉自己的页缓存，
//! 这里接着把空出来的页还给系统（施工 LW-15、LW-29）；一直安静到满 [`MERGE`]，才把每个开着的库的 `-wal` 合回主库、截成 0 字节，
//! 再回去等（施工 BO-3）。
//!
//! 照「库最后一次写（或者读，施工 LW-29）过去一阵子」算，不照「最后一个请求过去一阵子」（`settle.rs`）：一轮任务跑着的时候多半没有请求，会话照样
//! 一批批写索引、用量、回合库；照请求等，任务跑完以前一直合不上，被杀时那一截全留在 `-wal` 里。没有写的时候它一直睡着，不定时
//! 醒。一个进程只起一个（测试里一个进程开好几个核心，库的登记也是一个进程一份）。

use std::sync::Once;
use std::time::Duration;

use miyu_store::sqlite::idle::{self, Checkpointed};

use crate::TARGET;

/// 库最后一次提交以后多久算闲下来了、放掉页缓存：和请求的 `settle::QUIET` 一样长。一轮任务里等模型的那几秒也算。
pub const QUIET: Duration = Duration::from_secs(2);

/// 库最后一次提交以后多久才合（施工 BO-3）。比 [`QUIET`] 长得多，为的是一轮任务当中不合：合完以后每个库的下一次提交要
/// 先写新的日志头、同步一次，会话写一批要接连提交索引、用量两个库，工具等它写完才开始跑。真模型答一步通常是几秒到二十几秒，
/// 原来安静 2 秒就合，每一步都合一遍、下一步再多等两次同步。30 秒够盖住平常的一步；答得更久的一步合一次，那一步本身就是
/// 几十秒，多的十来毫秒不算什么。被杀时留在数据根里的 `-wal` 最多是 SQLite 自己做检查点之前的那一截（约 4 MB），下次开库合回。
pub const MERGE: Duration = Duration::from_secs(30);

/// 起合的线程；起过的不再起。起不来的记一行 `WARN wal checkpoints not started`：库照旧，`-wal` 照 SQLite 自己的检查点和
/// `journal_size_limit` 缩；关库时不合（施工 CS-9），留到下次开库读回。
pub fn spawn() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("miyu-wal".to_string())
            .spawn(|| {
                loop {
                    idle::wait_quiet(QUIET);
                    // 连接放掉了页缓存（施工 LW-15、LW-29，`heap.md`「怎么走」第 2 条）：空出来的页还给系统。一个连接都
                    // 没放成的（都正被用着、都没开成）没有新放掉的，不走这一遍堆。
                    if idle::shrink_all() > 0 {
                        miyu_heap::trim();
                    }
                    // 一轮任务当中不合（施工 BO-3）：再等到满 `MERGE`，这期间又有提交、读的回去重新等安静。
                    if !idle::stays_quiet(MERGE.saturating_sub(QUIET)) {
                        continue;
                    }
                    let done = idle::checkpoint_all();
                    // 合的时候又读进来几页，[`idle::checkpoint_all`] 合完再放了一遍。
                    if done.shrunk > 0 {
                        miyu_heap::trim();
                    }
                    report(&done);
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
