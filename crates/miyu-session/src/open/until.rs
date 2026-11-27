//! 造会话时核心要停了（施工 CS-9，`internal/blueprint/session.md`「造会话」）：照造到哪一步办，不留半个会话。
//!
//! 1. 还没开始写盘（读人格、配置、拼工具面这些）：当场不造了，交回 [`CreateError::Stopped`]，什么都没写；
//! 2. 存快照、建会话目录和空的第一段那一步已经交给阻塞线程：等它做完（写盘停不下来），再删掉目录（[`super::error::abandoned`]），
//!    交回同上。之后等模型目录备好的那一阵也照这一条；
//! 3. 起了 actor、在写造会话那一条（`session.created`）：等它写完，交回造好的会话，由调的一方停下它。
//!
//! 第 1 步不等，所以停的一方等的只有已经在路上的写盘（2026-11-27 复现：盘慢的时候建会话要几十秒，会话表的锁一直被它拿着，收尾
//! 的停会话一直等它）。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::watch;

use super::{Create, CreateError, Handle, created};

/// 同 [`super::create`]，`stop` 变成真时照这一页开头的三步办。发的一头没了的当不会停。
///
/// # Errors
///
/// 同 [`super::create`]；要停了、没造的是 [`CreateError::Stopped`]。
pub async fn create_until(
    setup: Create<'_>,
    stop: watch::Receiver<bool>,
) -> Result<Handle, CreateError> {
    let stop = Stop {
        stop: Some(stop),
        began: Arc::default(),
    };
    // 装箱：同 `create`，造会话这一段的 future 很大。
    let mut making = Box::pin(created(setup, stop.clone()));
    tokio::select! {
        biased;
        made = &mut making => made,
        () = stop.fired() => {
            // 停在开始写盘以前的 `await` 上：放下它，什么都没写。开始写了的照第 2、3 步做完。
            if stop.began() {
                making.await
            } else {
                Err(CreateError::Stopped)
            }
        }
    }
}

/// 造会话这一趟看的「要停了」：没有的（[`super::create`]）永远不停。
#[derive(Debug, Clone, Default)]
pub(super) struct Stop {
    stop: Option<watch::Receiver<bool>>,
    /// 开始写盘了（[`Stop::begin`]）。
    began: Arc<AtomicBool>,
}

impl Stop {
    /// 要开始写盘了：已经要停的交回假、不写；不然记下开始了、交回真，之后照第 2、3 步做完。
    pub(super) fn begin(&self) -> bool {
        if self.now() {
            return false;
        }
        self.began.store(true, Ordering::SeqCst);
        true
    }

    /// 开始写盘了没有。
    fn began(&self) -> bool {
        self.began.load(Ordering::SeqCst)
    }

    /// 这一刻要停了没有。
    fn now(&self) -> bool {
        self.stop.as_ref().is_some_and(|stop| *stop.borrow())
    }

    /// 等 `work` 做完、交回真；先要停了的（这一刻已经要停的也算）不等它、交回假。
    pub(super) async fn unless(&self, work: impl Future<Output = ()>) -> bool {
        if self.now() {
            return false;
        }
        tokio::select! {
            biased;
            () = work => true,
            () = self.fired() => false,
        }
    }

    /// 等到要停了。没有的、发的一头没了的一直等。
    async fn fired(&self) {
        let Some(mut stop) = self.stop.clone() else {
            return std::future::pending().await;
        };
        if stop.wait_for(|stopping| *stopping).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}
