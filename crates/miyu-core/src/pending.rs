//! 核心放锁之前要等完的后台写盘（施工 TMP-1 补2）：数据根里的东西只许拿着锁（`run/core.lock`）的那一份核心写。另起线程写的
//! （缓存目录里整理好的模型目录，[`crate::models::digest`]）在这里登记；核心走之前（正常停下、起不来）先等它们写完，再放锁。
//! 原来不等：锁放了它还在写，下一份核心起来、测试删数据根时撞上它，删掉的目录又被它一层层建回来。
//!
//! 写盘的线程是读目录的线程读完才起的（施工 TMP-3）：读目录的线程起来之前拿一个 [`Working`]，写盘的线程也拿一个、写完才放；
//! [`finish`] 等它们都放了。原来只等登记了的：核心起来不久就停下的，放锁时目录还没读完、登记里是空的，读完照样把删掉的数据根
//! 建回来（main b47ecacb3 的门禁）。照计数等，不照谁取走了登记：两处同时等的，各自都等到全部写完。
//!
//! 登记的写盘线程也连同一个 [`Working`] 记着，`join` 完才放（施工 TMP-3 补）：原来只有线程自己拿着的才算，[`hold`] 登记、自己
//! 不拿的，另一处 [`finish`] 先取走登记、还在等它时，这一处拿到空的就交回了（960c21421 的 Windows 门禁）。

use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

use crate::TARGET;

/// 登记了、还没等过的写盘线程，各带一个 [`Working`]（等完才放）。整个进程一份：起它们的地方（读目录的线程）手上没有核心。
static WRITES: Mutex<Vec<(JoinHandle<()>, Working)>> = Mutex::new(Vec::new());

/// 还在读目录、还在写盘的有几个（施工 TMP-3）。
static WORKING: Mutex<usize> = Mutex::new(0);

/// [`WORKING`] 减到 0 时叫醒等着的 [`finish`]。
static IDLE: Condvar = Condvar::new();

/// 一件还没做完的（施工 TMP-3）：读目录的、写盘的线程起来之前拿，交给那个线程，做完放；[`finish`] 等它们都放了。
pub(crate) struct Working(());

/// 开始一件：在起线程之前拿，免得线程还没跑起来就被当成做完了。
pub(crate) fn working() -> Working {
    *WORKING.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    Working(())
}

impl Drop for Working {
    fn drop(&mut self) {
        let mut working = WORKING.lock().unwrap_or_else(PoisonError::into_inner);
        *working = working.saturating_sub(1);
        if *working == 0 {
            IDLE.notify_all();
        }
    }
}

/// 登记一个写盘线程 `writing`：核心走之前等它（[`finish`]）。连同一个 [`Working`] 记着，等完才放（施工 TMP-3 补）。
pub(crate) fn hold(writing: JoinHandle<()>) {
    let held = working();
    registered().push((writing, held));
    // 叫醒照计数等着的：新登记的这个要有人取走、等完，计数才落得到 0。拿着计数的锁叫，免得它查完登记、还没睡下时叫丢了。
    let _working = WORKING.lock().unwrap_or_else(PoisonError::into_inner);
    IDLE.notify_all();
}

/// 登记里这时有的。
fn registered() -> std::sync::MutexGuard<'static, Vec<(JoinHandle<()>, Working)>> {
    WRITES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 等还在读、还在写的都做完（施工 TMP-3），登记了的写盘线程都等完（放锁之前调）；panic 了的记一行。会阻塞，在异步代码里放进
/// 阻塞线程。
///
/// 先取走登记了的、一个一个等完（施工 TMP-3 补）；取不到了再照计数等到 0：别处取走的，等它们那边等完；读目录的读完又登记了新
/// 的，接着取（[`hold`] 登记时叫醒等着的）。两处同时调的，各自都等到全部写完。
pub(crate) fn finish() {
    loop {
        let writes = std::mem::take(&mut *registered());
        if writes.is_empty() {
            let mut working = WORKING.lock().unwrap_or_else(PoisonError::into_inner);
            while *working > 0 && registered().is_empty() {
                working = IDLE.wait(working).unwrap_or_else(PoisonError::into_inner);
            }
            if *working == 0 && registered().is_empty() {
                return;
            }
            continue;
        }
        for (writing, held) in writes {
            if writing.join().is_err() {
                tracing::error!(target: TARGET, "background write panicked");
            }
            drop(held);
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
