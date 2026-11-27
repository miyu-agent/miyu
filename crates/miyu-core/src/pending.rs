//! 核心放锁之前要等完的后台写盘（施工 TMP-1 补2）：数据根里的东西只许拿着锁（`run/core.lock`）的那一份核心写。另起线程写的
//! （缓存目录里整理好的模型目录，[`crate::models::digest`]）经 [`write()`] 起、登记在这里；核心走之前（正常停下、起不来）[`close`]：
//! 关上门，等已经起了的写完，再放锁。原来不等：锁放了它还在写，下一份核心起来、测试删数据根时撞上它，删掉的目录又被它一层层
//! 建回来。
//!
//! 写盘的线程是读目录的线程读完才起的（施工 TMP-3）：原来走之前连还在读的也等，读完起的写盘才不会落在放锁以后。压力下读一份
//! 目录要一两分钟，收到停的信号的核心一直等它，一次 SIGTERM 停不下（2026-11-27 基准测试 B1 第 5 次）。现在照门（施工 CS-8）：
//! [`close`] 先关门，关了以后读完的不再起写盘的线程，所以不用等还在读的；读的只读，进程没了跟着没了。
//!
//! 读目录的线程照旧拿一个 [`Working`]：测试的夹具删临时目录之前照 `finish` 连还在读的一起等完（`models::settled`），不关门，
//! 同一个测试进程里后面的核心照常写。

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use crate::TARGET;

/// 还在读的、起了的写盘线程、关没关门（施工 CS-8）。整个进程一份 [`PENDING`]：起它们的地方（读目录的线程）手上没有核心。
/// 测试另造自己的一份，看关门的事不碍着同一个进程里别的测试。
pub(crate) struct Pending {
    state: Mutex<State>,
    /// 有一件做完了、登记了新的写盘线程：叫醒等着的。
    changed: Condvar,
}

/// [`Pending`] 里锁着的那一份：一把锁管全部，等的一方照它判，叫醒不会丢。
struct State {
    /// 还在读目录的有几个（[`Working`]）。
    reading: usize,
    /// 过了门、还没写完的写盘线程有几个（[`Writing`]）：起了线程、还没登记的也算。
    writing: usize,
    /// 起了的写盘线程，等的时候 `join`，panic 了的记一行。
    threads: Vec<JoinHandle<()>>,
    /// 关了门（[`close`]）：之后不再起写盘的线程。
    closed: bool,
}

/// 整个进程的那一份。
static PENDING: Pending = Pending::new();

/// 一件还在读的（施工 TMP-3）：读目录的线程起来之前拿，交给那个线程，读完放；`finish` 等它们都放了。
pub(crate) struct Working(&'static Pending);

/// 一个过了门的写盘线程（施工 CS-8）：[`write()`] 交给它，写完放；[`close`]、`finish` 等它们都放了。
pub(crate) struct Writing(&'static Pending);

impl Pending {
    /// 空的：没有在读的、没有在写的，门开着。
    pub(crate) const fn new() -> Pending {
        Pending {
            state: Mutex::new(State {
                reading: 0,
                writing: 0,
                threads: Vec::new(),
                closed: false,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 开始读一件：在起线程之前拿，免得线程还没跑起来就被当成读完了。
    pub(crate) fn working(&'static self) -> Working {
        self.lock().reading += 1;
        Working(self)
    }

    /// 门开着的，照 `start` 起一个写盘线程（交给它的 [`Writing`] 要拿到写完）、登记下来；门关了的不起，`start` 不调。起没起
    /// 来都交回。线程在锁外面起：起不来时 `start` 丢掉的 [`Writing`] 要再拿这把锁。
    pub(crate) fn write(&'static self, start: impl FnOnce(Writing) -> Option<JoinHandle<()>>) {
        let writing = {
            let mut state = self.lock();
            if state.closed {
                return;
            }
            state.writing += 1;
            Writing(self)
        };
        if let Some(thread) = start(writing) {
            self.lock().threads.push(thread);
            self.changed.notify_all();
        }
    }

    /// 等：`close` 的先关门，等过了门的写完，不等还在读的；不关门的连还在读的一起等（读完起的写盘也等完）。登记了的线程都
    /// `join`，panic 了的记一行。会阻塞。
    fn wait(&self, close: bool) {
        let mut state = self.lock();
        state.closed |= close;
        loop {
            let threads = std::mem::take(&mut state.threads);
            if !threads.is_empty() {
                drop(state);
                for thread in threads {
                    if thread.join().is_err() {
                        tracing::error!(target: TARGET, "background write panicked");
                    }
                }
                state = self.lock();
                continue;
            }
            if state.writing == 0 && (close || state.reading == 0) {
                return;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// 关上门，等过了门的写完（见 [`close`]）。
    pub(crate) fn close(&self) {
        self.wait(true);
    }

    /// 门开着，连还在读的一起等完（见 `finish`）。
    #[cfg(any(test, feature = "testkit"))]
    pub(crate) fn finish(&self) {
        self.wait(false);
    }
}

impl Drop for Working {
    fn drop(&mut self) {
        let pending = self.0;
        let mut state = pending.lock();
        state.reading = state.reading.saturating_sub(1);
        drop(state);
        pending.changed.notify_all();
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        let pending = self.0;
        let mut state = pending.lock();
        state.writing = state.writing.saturating_sub(1);
        drop(state);
        pending.changed.notify_all();
    }
}

/// 开始读一件（整个进程的那一份）：见 [`Pending::working`]。
pub(crate) fn working() -> Working {
    PENDING.working()
}

/// 门开着的起一个写盘线程、登记下来（整个进程的那一份）：见 [`Pending::write`]。
pub(crate) fn write(start: impl FnOnce(Writing) -> Option<JoinHandle<()>>) {
    PENDING.write(start);
}

/// 核心走之前、放锁之前调（施工 CS-8）：关上门，等已经起了的写盘线程都写完；还在读目录的不等，它读完也不再起写盘的线程。会阻塞，
/// 在异步代码里放进阻塞线程。
pub(crate) fn close() {
    PENDING.close();
}

/// 等还在读的、还在写的都做完，读完起的写盘也等完（施工 TMP-3），不关门：测试的夹具删临时目录之前调（`models::settled`）。会
/// 阻塞。
#[cfg(any(test, feature = "testkit"))]
pub(crate) fn finish() {
    PENDING.finish();
}

/// 走之前的收尾（施工 TMP-1 补2，`core.md`「停下」第 6 条）：等起来时在后台清回收处的 `purging`、清缓存的 `sweeping` 清完，
/// 再关门、等已经起了的写盘线程写完（[`close`]）。交给 [`crate::serve_then`]，在放锁之前做。
pub(crate) async fn closing(purging: JoinTask, sweeping: JoinTask) {
    if let Err(error) = purging.await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "trash purge panicked");
    }
    if let Err(error) = sweeping.await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "cache sweep panicked");
    }
    if let Err(error) = tokio::task::spawn_blocking(close).await {
        tracing::error!(target: TARGET, panic = error.is_panic(), "background writes not awaited");
    }
}

/// 起来时在后台起的一件（清回收处、清缓存）。
type JoinTask = tokio::task::JoinHandle<()>;

#[cfg(test)]
mod tests;
