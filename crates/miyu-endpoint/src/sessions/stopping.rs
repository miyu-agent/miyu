//! 核心要停了（施工 CS-9，`internal/blueprint/protocol.md`「会话表」）：停会话（[`super::Sessions::stop_all`]）不再等正在造的
//! 会话手里的表的锁。
//!
//! 原来造会话从找人格一直到 `session.created` 落盘都拿着会话表的锁，收尾的停会话要先拿这把锁：盘慢的时候
//! 建会话要几十秒，停会话就跟着等几十秒（2026-11-27 复现：每次同步拖 4 秒，SIGTERM 以后 17.6 秒才记出会话停了）。现在：
//!
//! 1. 停的一方先说要停了（[`Stopping::begin`]），再拿锁；
//! 2. 造会话的一方读的那几步（补去重的编号、找预设人格）照 [`Stopping::unless`] 当场不读了；交给 `miyu_session` 的那一段
//!    另起一个任务（[`Stopping::make`]），要停了就不等它、交回 [`Made::Stopped`]，放开锁。那个任务照
//!    [`miyu_session::create_until`]：没开始写盘的不写，建好目录还没写 `session.created` 的删掉目录，在写的写完；
//! 3. 停的一方拿到锁以后，和停在跑的会话一起等这些任务做完（[`Stopping::settle`]）：造成了的是一个完整的会话，有计划地停下，
//!    和别的会话一样。头收到的是 `session_stopped`，重连以后照同一个命令编号再造，交回的就是它（`sessions/creating.rs` 的去重）。
//!
//! 不留半个会话：要么什么都没写，要么目录删了，要么 `session.created` 写完了。停的一方等完了才往下走，放锁以前不再有在写的；
//! 只有收尾过了总时限当场退出（`miyu-core` 的 `stops.rs`）时才可能留下只有空的第一段的目录，和崩了一样：载入是 `not_found`，
//! 列会话、补去重的编号都跳过它。

use std::sync::{Arc, PoisonError};

use tokio::sync::watch;
use tokio::task::JoinHandle;

use miyu_session::{CreateError, Handle, create_until};

use super::Setup;
use crate::Core;

/// 要停了没有，停的时候还在造的会话。
#[derive(Debug)]
pub(super) struct Stopping {
    /// 要停了：真了就不再变回去。
    stop: watch::Sender<bool>,
    /// 要停的时候还在造的：停的一方等它们做完、停下造成了的。
    left: std::sync::Mutex<Vec<JoinHandle<Result<Handle, CreateError>>>>,
}

impl Default for Stopping {
    fn default() -> Stopping {
        Stopping {
            stop: watch::Sender::new(false),
            left: std::sync::Mutex::default(),
        }
    }
}

/// 交给 `miyu_session` 造的结果。
pub(super) enum Made {
    /// 造好了。
    Done(Handle),
    /// 要停了：没等它造完，留给停的一方。
    Stopped,
    /// 造不成。
    Failed(CreateError),
    /// 造的那个任务崩了。
    Panicked,
}

impl Stopping {
    /// 要停了：之后造会话的不等写盘、不再造。
    pub(super) fn begin(&self) {
        self.stop.send_replace(true);
    }

    /// 这一刻要停了没有。
    pub(super) fn now(&self) -> bool {
        *self.stop.borrow()
    }

    /// 做 `work`、交回它的结果；做完以前要停了的放下它、交回空的。`work` 只许读：放下它不留下写了一半的。
    pub(super) async fn unless<T>(&self, work: impl Future<Output = T>) -> Option<T> {
        let mut stop = self.stop.subscribe();
        tokio::select! {
            biased;
            done = work => Some(done),
            // 发的一头在 `self` 里，等的时候不会没了。
            _ = stop.wait_for(|stopping| *stopping) => None,
        }
    }

    /// 照 `setup` 造一个会话：另起一个任务交给 [`create_until`]，要停了就不等它（留给 [`Stopping::settle`]）。
    pub(super) async fn make(&self, core: &Arc<Core>, setup: Setup) -> Made {
        let (core, told) = (Arc::clone(core), self.stop.subscribe());
        let mut task = tokio::spawn(async move {
            let mut setup = setup;
            create_until(setup.create(&core), told).await
        });
        let mut stop = self.stop.subscribe();
        tokio::select! {
            biased;
            joined = &mut task => match joined {
                Ok(Ok(handle)) => Made::Done(handle),
                Ok(Err(CreateError::Stopped)) => Made::Stopped,
                Ok(Err(error)) => Made::Failed(error),
                Err(_) => Made::Panicked,
            },
            _ = stop.wait_for(|stopping| *stopping) => {
                self.left.lock().unwrap_or_else(PoisonError::into_inner).push(task);
                Made::Stopped
            }
        }
    }

    /// 等要停的时候还在造的会话做完：造成了的有计划地停下。要在拿到过表的锁以后调：那以后不会再有留下来的（造会话的一方
    /// 拿着锁时留下、放锁，之后拿到锁的看到要停了就不造）。
    pub(super) async fn settle(&self) {
        let left = std::mem::take(&mut *self.left.lock().unwrap_or_else(PoisonError::into_inner));
        for task in left {
            match task.await {
                Ok(Ok(handle)) => {
                    if handle.stop().await.is_err() {
                        tracing::debug!(target: "miyu::endpoint", "created session already stopped");
                    }
                }
                Ok(Err(CreateError::Stopped)) => {}
                Ok(Err(error)) => {
                    tracing::warn!(target: "miyu::endpoint", error = %error, "create failed");
                }
                Err(_) => tracing::error!(target: "miyu::endpoint", "create panicked"),
            }
        }
    }
}

#[cfg(test)]
mod tests;
