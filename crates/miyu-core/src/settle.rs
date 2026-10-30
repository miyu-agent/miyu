//! 一阵请求过去、安静下来以后把堆里空着的还给系统（施工 V-5 中，`docs/blueprint/heap.md`「还内存」）。
//!
//! 头一连上就是一阵：读配置、列模型、读人，几百 KB 的回应拼完就放掉，放掉的留在 glibc 手里不还，核心空闲时就一直多占
//! 几 MB（施工 V-5 中量过：连上终端以后匿名内存多 3.3 MB，活着的数据只多 40 KB）。所以每来一个请求，连接叫一声
//! （`miyu_endpoint::Activity`）；叫过以后 [`QUIET`] 这么久再没有请求，还一次，再做一件安静下来以后的事（核心里是看内存
//! 涨得快不快，施工 V-6 下，`memlog.rs`）。没请求的时候这里一直睡着，不定时醒。

use std::pin::Pin;
use std::time::Duration;

use miyu_endpoint::Activity;

use crate::TARGET;

/// 最后一个请求过去多久算安静下来了。
pub const QUIET: Duration = Duration::from_secs(2);

/// 安静下来以后再做的一件事（施工 V-6 下）。
pub type Then = Box<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// 起盯着请求的任务：每次安静下来调一次 `release`（核心里是放掉读好的字、再 [`miyu_heap::trim`]，在阻塞线程里调），再等
/// `then` 做完。要在 tokio 的运行时里调。
pub fn spawn(
    activity: &Activity,
    quiet: Duration,
    release: impl Fn() -> bool + Clone + Send + 'static,
    then: Then,
) -> tokio::task::JoinHandle<()> {
    let mut changes = activity.subscribe();
    tokio::spawn(async move {
        loop {
            if changes.changed().await.is_err() {
                return;
            }
            while let Ok(Ok(())) = tokio::time::timeout(quiet, changes.changed()).await {}
            if !settled(tokio::task::spawn_blocking(release.clone()).await) {
                return;
            }
            then().await;
        }
    })
}

/// 还了一次以后怎么收，交回要不要接着盯。还了的记一行 `DEBUG`；崩了的记 `ERROR heap settle panicked`，接着盯；被取消的是运行时
/// 在收尾（停核心时阻塞任务排不上，施工 BUG-1：以前也当成崩了打 `ERROR`），记一行 `DEBUG`，不再盯。
fn settled(result: Result<bool, tokio::task::JoinError>) -> bool {
    match result {
        Ok(released) => {
            tracing::debug!(target: TARGET, released, "heap settled");
            true
        }
        Err(error) if error.is_panic() => {
            tracing::error!(target: TARGET, panic = true, "heap settle panicked");
            true
        }
        Err(_) => {
            tracing::debug!(target: TARGET, "heap settle cancelled");
            false
        }
    }
}

#[cfg(test)]
mod tests;
