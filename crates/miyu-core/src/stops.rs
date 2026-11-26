//! 停的信号和收尾的总时限（施工 CS-6，`internal/blueprint/core.md`「停下」第 4、5、11 条）。
//!
//! 信号起来时就装（[`watch`]，在开套接字、说「好了」以前）：原来等开始接连接时才装，起来那一段收到的 SIGTERM 照系统的默认当场
//! 杀掉，收尾那一段第二次来的被吞掉（tokio 装上以后不再还给系统的默认）。现在：
//!
//! 1. 第一次（SIGTERM、Ctrl+C 都算）交给 [`crate::serve()`] 有计划地停：起来那一段收到的，等开始接连接时当场停；
//! 2. 之后再来一次（不管哪一种），记一行 `WARN second stop signal, exiting now`、把运行日志写到盘上，退出码 1 当场退出
//!    （2026-11-27 项目主人定）；
//! 3. 收尾一开头（[`Begun::now`]：信号、头叫停、空闲、数据根没了都算）起表，过了 [`LIMIT`] 还没做完，记一行
//!    `WARN stopping took too long, exiting now`，同样当场退出。
//!
//! 当场退出不跑收尾剩下的：锁、套接字的事交给系统（进程没了锁就放，留下的套接字文件下一个核心起来时清），长住的子进程、后台
//! 命令照「子进程随核心退出」由看门的收掉（`core.md`）。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Notify, oneshot};

use crate::TARGET;

/// 收尾最多做多久（施工 CS-6）：30 秒。停扩展、MCP 服务各等 5 秒，记忆写完、清回收处一般几秒；比 `miyu core stop` 等核心退的
/// 60 秒短，人敲的「停」等得到它；比 systemd 默认的 90 秒（`TimeoutStopSec`）短，服务管理器不用自己去杀。
const LIMIT: Duration = Duration::from_secs(30);

/// 起来时装好的停的信号：第一次交给 [`Stops::first`]。
pub(crate) struct Stops {
    /// 第一次来了。
    first: oneshot::Receiver<()>,
    /// 收尾开始了：起表。
    begun: Begun,
}

/// 收尾开始了，从这一刻起算总时限。
#[derive(Clone)]
pub(crate) struct Begun(Arc<Notify>);

impl Begun {
    /// 收尾开始了。叫过一次以后再叫不算。
    pub(crate) fn now(&self) {
        self.0.notify_one();
    }
}

impl Stops {
    /// 拆成第一次停的信号（交给 `serve` 当 `stop`）和起表的那一头。
    pub(crate) fn split(self) -> (impl Future<Output = ()>, Begun) {
        let first = self.first;
        let first = async move {
            // 装不上信号（发信号的任务没了）的当它不会来。
            if first.await.is_err() {
                std::future::pending::<()>().await;
            }
        };
        (first, self.begun)
    }
}

/// 装停的信号、起看总时限的任务（见这一页开头）。`flush` 是强退前把运行日志写到盘上的把手。要在 tokio 运行时里调。
pub(crate) fn watch(flush: miyu_log::Flush) -> Stops {
    let mut signals = Signals::install();
    let (arrived, first) = oneshot::channel();
    let begun = Begun(Arc::new(Notify::new()));
    let stopped = flush.clone();
    tokio::spawn(async move {
        signals.next().await;
        // 收的一头没了（核心已经在走了）也接着等第二次。
        let _arrived = arrived.send(());
        signals.next().await;
        exit_now(&stopped, Why::Again);
    });
    let started = Arc::clone(&begun.0);
    tokio::spawn(async move {
        started.notified().await;
        tokio::time::sleep(limit()).await;
        exit_now(&flush, Why::TooLong);
    });
    Stops { first, begun }
}

/// 为什么当场退。
#[derive(Debug, Clone, Copy)]
enum Why {
    /// 收尾时又来一次停的信号。
    Again,
    /// 收尾过了总时限。
    TooLong,
}

/// 记一行、把运行日志写到盘上，退出码 1 当场退出。
fn exit_now(flush: &miyu_log::Flush, why: Why) -> ! {
    match why {
        Why::Again => tracing::warn!(target: TARGET, "second stop signal, exiting now"),
        Why::TooLong => tracing::warn!(target: TARGET, "stopping took too long, exiting now"),
    }
    flush.sync();
    std::process::exit(1)
}

/// 这一次的总时限：出厂 [`LIMIT`]；打开 `testkit` 的构建另认测试口子 `MIYU_TEST_STOP_LIMIT_MS`（毫秒）。
fn limit() -> Duration {
    #[cfg(any(test, feature = "testkit"))]
    let limit = std::env::var("MIYU_TEST_STOP_LIMIT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(LIMIT, Duration::from_millis);
    #[cfg(not(any(test, feature = "testkit")))]
    let limit = LIMIT;
    limit
}

/// 装好的几个停的信号：Ctrl+C（SIGINT），Unix 上还有 SIGTERM。拉起的核心自成一个进程组，终端里按 Ctrl+C 打不到它，要停它得明着
/// 发。装不上的当不会来：记一条 `WARN`，照样等装上了的（施工 4-9 再补三上）；都装不上的一直等下去。
struct Signals {
    #[cfg(unix)]
    terminate: Option<tokio::signal::unix::Signal>,
    #[cfg(unix)]
    interrupt: Option<tokio::signal::unix::Signal>,
    #[cfg(windows)]
    interrupt: Option<tokio::signal::windows::CtrlC>,
}

impl Signals {
    /// 当场装上：从这一刻起来的都记着，还没人等也不丢。
    fn install() -> Signals {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Signals {
                terminate: installed(signal(SignalKind::terminate()), Kind::Terminate),
                interrupt: installed(signal(SignalKind::interrupt()), Kind::Interrupt),
            }
        }
        #[cfg(windows)]
        Signals {
            interrupt: installed(tokio::signal::windows::ctrl_c(), Kind::Interrupt),
        }
    }

    /// 等下一个。
    async fn next(&mut self) {
        #[cfg(unix)]
        tokio::select! {
            () = arrived(self.terminate.as_mut().map(|one| one.recv())) => {}
            () = arrived(self.interrupt.as_mut().map(|one| one.recv())) => {}
        }
        #[cfg(windows)]
        arrived(self.interrupt.as_mut().map(|one| one.recv())).await;
    }
}

/// 哪一种停的信号。
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// SIGTERM（Unix）。
    #[cfg_attr(windows, expect(dead_code, reason = "Windows 上没有 SIGTERM"))]
    Terminate,
    /// Ctrl+C：Unix 上是 SIGINT。
    Interrupt,
}

/// 装上了的留下；装不上的照 `kind` 记一条 `WARN`，当没有。
fn installed<T>(signal: std::io::Result<T>, kind: Kind) -> Option<T> {
    signal
        .inspect_err(|error| match kind {
            Kind::Terminate => {
                tracing::warn!(target: TARGET, error = %error, "SIGTERM not watched")
            }
            Kind::Interrupt => tracing::warn!(target: TARGET, error = %error, "Ctrl+C not watched"),
        })
        .ok()
}

/// 等一个信号来：没装上（`None`）的、装上了又断了的当不会来，一直等下去。
async fn arrived(recv: Option<impl Future<Output = Option<()>>>) {
    let came = match recv {
        Some(recv) => recv.await.is_some(),
        None => false,
    };
    if !came {
        std::future::pending::<()>().await;
    }
}

#[cfg(test)]
mod tests;
