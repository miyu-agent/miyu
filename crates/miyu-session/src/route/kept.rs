//! 会话留着的编好的历史和它什么时候放（施工 LW-3，`internal/blueprint/session/actor.md`「8. 经路由请求」第 5 款）：主请求编码
//! 时经 [`Kept::encode`] 用会话那一份 [`Memo`]（施工 V-42）；没有主请求满 [`KEEP`] 就放掉，下一次从头编，发出去的字节不变。
//!
//! 计时不轮询：每次编码以后记下这一刻，这时没有定时在等的才挂一个，睡到「最后一次用的时刻加 [`KEEP`]」。醒来以后在锁里再看
//! 最后一次用的时刻：其间又用过的接着睡到新的时刻，没用过的放掉、放下记号，出了锁再把内存还给系统（[`crate::heap::trim_soon`]）。
//! 所以一个会话任何时刻最多一个定时，没有主请求的会话没有定时。定时只拿弱引用：会话退下、路由放掉以后，它醒来就结束，不把这一份
//! 钉在内存里；核心退出时运行时停了，它跟着被取消。

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use tokio::time::Instant;

use miyu_drivers::Memo;

use crate::TARGET;

/// 没有主请求满这么久，放掉留着的历史：3 分钟。和会话闲够 3 分钟退下（`protocol.md`「会话表」第 9 条）是同一个数：窗口开着、
/// 会话退不下的时候，闲着的会话也像退下了一样不留这一份。不做成配置项（施工单 LW-3「不做什么」）：它只是缓存，放掉以后下一次
/// 请求多编十几毫秒，发出去的字节不变，没有要人来选的。
pub(crate) const KEEP: Duration = Duration::from_secs(180);

/// 一个会话留着的编好的历史，和放掉它的定时。路由里一个会话一份，放在 `Arc` 里：定时的任务只拿弱引用。
#[derive(Debug, Default)]
pub(super) struct Kept {
    held: Mutex<Held>,
}

/// 锁里的那一份：留着的，和最后一次用的时刻。两样在同一把锁里，定时到点和下一次编码撞在一起时谁先拿到锁都对得上（施工单
/// LW-3「风险」第 1 条）。
#[derive(Debug, Default)]
struct Held {
    memo: Memo,
    /// 最后一次编码用它的时刻；有的说明有一个定时在等，没有的说明没有。
    used: Option<Instant>,
}

impl Kept {
    /// 拿着留着的那一份编一次（主请求编码时调，要在 tokio 的运行时里）：编完记下这一刻，没有定时在等的挂一个。
    pub(super) fn encode<T>(self: &Arc<Self>, encode: impl FnOnce(&mut Memo) -> T) -> T {
        let mut held = self.lock();
        let encoded = encode(&mut held.memo);
        let now = Instant::now();
        if held.used.replace(now).is_none() {
            drop(tokio::spawn(release(Arc::downgrade(self), now + KEEP)));
        }
        encoded
    }

    /// 拿着锁里的那一份：编到一半 panic 过的（锁坏了），留着的不一定齐，整份扔掉重来；最后一次用的时刻和定时对得上，不动。
    fn lock(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|poisoned| {
            let mut held = poisoned.into_inner();
            held.memo.clear();
            self.held.clear_poison();
            held
        })
    }

    /// 一条都没留（测试看）。
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.lock().memo.is_empty()
    }

    /// 有没有一个定时在等（测试看）。
    #[cfg(test)]
    fn waiting(&self) -> bool {
        self.lock().used.is_some()
    }
}

/// 定时：睡到 `due`，没再用过的放掉。路由已经没了的直接结束。每一圈醒来才把弱引用升成强的，睡的时候不拿着它。
async fn release(kept: Weak<Kept>, mut due: Instant) {
    loop {
        tokio::time::sleep_until(due).await;
        let Some(kept) = kept.upgrade() else {
            return;
        };
        let mut held = kept.lock();
        let Some(used) = held.used else {
            return;
        };
        if used + KEEP > Instant::now() {
            due = used + KEEP;
            continue;
        }
        let messages = held.memo.len();
        // `clear` 换成一份新的 `Memo`：原样的消息、编好的片段连同占着的容量一起放掉。
        held.memo.clear();
        held.used = None;
        drop(held);
        tracing::debug!(target: TARGET, messages, "memo released");
        crate::heap::trim_soon();
        return;
    }
}

#[cfg(test)]
mod tests;
