//! 一阵请求过去、安静下来以后把堆里空着的还给系统（施工 V-5 中，`docs/blueprint/heap.md`「还内存」）。
//!
//! 头一连上就是一阵：读配置、列模型、读人，几百 KB 的回应拼完就放掉，放掉的留在 glibc 手里不还，核心空闲时就一直多占
//! 几 MB（施工 V-5 中量过：连上终端以后匿名内存多 3.3 MB，活着的数据只多 40 KB）。所以每来一个请求，连接叫一声
//! （`miyu_endpoint::Activity`）；叫过以后 [`QUIET`] 这么久再没有请求，还一次。没请求的时候这里一直睡着，不定时醒。

use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::Activity;

use crate::TARGET;

/// 最后一个请求过去多久算安静下来了。
pub const QUIET: Duration = Duration::from_secs(2);

/// 起盯着请求的任务：每次安静下来调一次 `release`（核心里是 [`miyu_heap::trim`]，在阻塞线程里调）。要在 tokio 的运行时
/// 里调。
pub fn spawn(
    activity: Arc<Activity>,
    quiet: Duration,
    release: fn() -> bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            activity.wait().await;
            while tokio::time::timeout(quiet, activity.wait()).await.is_ok() {}
            match tokio::task::spawn_blocking(release).await {
                Ok(released) => tracing::debug!(target: TARGET, released, "heap settled"),
                Err(error) => {
                    tracing::error!(target: TARGET, error = %error, "heap settle panicked")
                }
            }
        }
    })
}

#[cfg(test)]
mod tests;
