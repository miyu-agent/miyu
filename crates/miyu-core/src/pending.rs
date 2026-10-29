//! 核心放锁之前要等完的后台写盘（施工 TMP-1 补2）：数据根里的东西只许拿着锁（`run/core.lock`）的那一份核心写。另起线程写的
//! （缓存目录里整理好的模型目录，[`crate::models::digest`]）在这里登记；核心走之前（正常停下、起不来）先等它们写完，再放锁。
//! 原来不等：锁放了它还在写，下一份核心起来、测试删数据根时撞上它，删掉的目录又被它一层层建回来。

use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use crate::TARGET;

/// 登记了、还没等过的写盘线程。整个进程一份：起它们的地方（读目录的线程）手上没有核心。
static WRITES: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

/// 登记一个写盘线程 `writing`：核心走之前等它（[`finish`]）。
pub(crate) fn hold(writing: JoinHandle<()>) {
    WRITES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(writing);
}

/// 等登记了的写盘线程都写完（放锁之前调）；panic 了的记一行。会阻塞，在异步代码里放进阻塞线程。
pub(crate) fn finish() {
    let writes = std::mem::take(&mut *WRITES.lock().unwrap_or_else(PoisonError::into_inner));
    for writing in writes {
        if writing.join().is_err() {
            tracing::error!(target: TARGET, "background write panicked");
        }
    }
}

/// 走之前的收尾（施工 TMP-1 补2，`core.md`「停下」第 6 条）：等起来时在后台清回收处的 `purging`、清缓存的 `sweeping` 清完，
/// 再等另起线程写盘的写完。交给 [`crate::serve_then`]，在放锁之前做。
pub(crate) async fn closing(purging: JoinTask, sweeping: JoinTask) {
    if let Err(error) = purging.await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "trash purge panicked");
    }
    if let Err(error) = sweeping.await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "cache sweep panicked");
    }
    if let Err(error) = tokio::task::spawn_blocking(finish).await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "background writes not awaited");
    }
}

/// 起来时在后台起的一件（清回收处、清缓存）。
type JoinTask = tokio::task::JoinHandle<()>;

#[cfg(test)]
mod tests;
