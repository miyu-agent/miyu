//! 等核心等多久（施工 CS-6，`internal/blueprint/ipc.md`「等多久」）：拉起核心、等拉起锁、等核心能连上、等握手的回应，都照
//! 同一份耐心：过了 [`Patience::slow`]（3 秒）还没好，先叫一声「慢了」（头照它说「核心还在启动…」），接着等，最多等
//! [`Patience::limit`]（10 分钟）。2026-11-27 项目主人定：核心活着、只是起得慢的接着等，留一个很长的上限；核心退了、说了起不来、
//! 人按了 Ctrl+C 的当场结束。
//!
//! 能看状态的先看状态（管道读到了头、拉起的进程退了、锁放了），上限只兜「活着却一直不动」的那一种。原来只照时间判，
//! 60 秒一到就报超时，冷启动慢的机器上起得来的核心也被说成起不来。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::time::Instant;

/// 轮询的几处（拉起锁、等核心能连上）每次歇多久。
pub(crate) const PAUSE: Duration = Duration::from_millis(50);

/// 等核心的耐心。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patience {
    /// 过了这么久还没好，叫一声「慢了」。
    pub slow: Duration,
    /// 最多等这么久。
    pub limit: Duration,
}

impl Patience {
    /// 出厂的：3 秒叫一声，最多 10 分钟（2026-11-27 项目主人定）。
    pub const DEFAULT: Patience = Patience {
        slow: Duration::from_secs(3),
        limit: Duration::from_secs(600),
    };

    /// 这一次用的：出厂的那一份。打开 `testkit` 的构建（`cargo test` 编出来的主程序）另认两个测试口子
    /// `MIYU_TEST_WAIT_SLOW_MS`、`MIYU_TEST_WAIT_LIMIT_MS`（毫秒）：真的主程序、真的核心的测试不用真等十分钟。发出去的程序
    /// 不读它们。
    #[must_use]
    pub fn current() -> Patience {
        #[cfg(any(test, feature = "testkit"))]
        let patience = testing(Patience::DEFAULT);
        #[cfg(not(any(test, feature = "testkit")))]
        let patience = Patience::DEFAULT;
        patience
    }
}

/// 测试口子盖过 `patience`：没设的、读不懂的照原样。
#[cfg(any(test, feature = "testkit"))]
fn testing(patience: Patience) -> Patience {
    let millis = |name: &str| {
        std::env::var(name)
            .ok()?
            .parse::<u64>()
            .ok()
            .map(Duration::from_millis)
    };
    Patience {
        slow: millis("MIYU_TEST_WAIT_SLOW_MS").unwrap_or(patience.slow),
        limit: millis("MIYU_TEST_WAIT_LIMIT_MS").unwrap_or(patience.limit),
    }
}

/// 慢了叫谁：一次等最多叫一次。头照它在标准错误上说一句，网页照它把状态换成「还在启动」。
pub type Slow<'a> = &'a (dyn Fn() + Send + Sync);

/// 一次等的钟：从造它的那一刻算，几段等（拉起锁、拉起、等能连上）共用一个，「慢了」只叫一次、上限是总的。
pub(crate) struct Clock<'a> {
    /// 到这一刻叫一声。
    slow_at: Instant,
    /// 到这一刻不等了。
    deadline: Instant,
    /// 叫过了。
    told: AtomicBool,
    /// 叫谁。
    slow: Slow<'a>,
}

impl<'a> Clock<'a> {
    /// 照 `patience` 从这一刻算，慢了叫 `slow`。
    pub(crate) fn new(patience: Patience, slow: Slow<'a>) -> Clock<'a> {
        let now = Instant::now();
        Clock {
            slow_at: now + patience.slow,
            deadline: now + patience.limit,
            told: AtomicBool::new(false),
            slow,
        }
    }

    /// 到了该叫的时候、还没叫过的，叫一声。
    pub(crate) fn look(&self) {
        if Instant::now() >= self.slow_at && !self.told.swap(true, Ordering::Relaxed) {
            (self.slow)();
        }
    }

    /// 过了上限。
    pub(crate) fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// 上限那一刻。
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    /// 歇 [`PAUSE`]，顺手看要不要叫一声。
    pub(crate) async fn pause(&self) {
        tokio::time::sleep(PAUSE).await;
        self.look();
    }

    /// 等到 `slow_at` 叫一声；叫过了的一直不回（给 `select!` 用）。
    pub(crate) async fn tell(&self) {
        if self.told.load(Ordering::Relaxed) {
            return std::future::pending().await;
        }
        tokio::time::sleep_until(self.slow_at).await;
        self.look();
        std::future::pending().await
    }

    /// 等 `future`，慢了叫一声，过了上限交回 `None`（`future` 当场放下）。
    pub(crate) async fn within<F: Future>(&self, future: F) -> Option<F::Output> {
        tokio::select! {
            out = future => Some(out),
            () = self.tell() => None,
            () = tokio::time::sleep_until(self.deadline) => None,
        }
    }
}

/// 照 `patience` 等 `future`：过了 `slow` 叫一声 `slow`，过了 `limit` 交回 `None`，`future` 当场放下（施工 CS-6）。头等握手的回应
/// 用它：连上了却一直没人回的（核心被 `SIGSTOP`、卡死），不一直空等。
pub async fn patiently<F: Future>(
    future: F,
    patience: Patience,
    slow: Slow<'_>,
) -> Option<F::Output> {
    Clock::new(patience, slow).within(future).await
}

#[cfg(test)]
mod tests;
