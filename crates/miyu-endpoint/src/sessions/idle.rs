//! 闲够了的会话退下（`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条，施工 V-2 再补）：会话表第一次放进会话时起一个
//! 任务，隔一阵拿着表的锁问一遍。没有别的把手、没人订阅、没有在跑的回合的，问它的 actor 闲够了没有（`Handle::retire`，它照
//! 自己的账答）；退下了的、已经停了的从表里拿掉，下次用到再载入。actor 退出时把放下的内存还给系统（`miyu-heap`）。
//!
//! 拿着锁问：这一刻谁也拿不到新的把手，退下和再载入不会同时有两个写者。下一次什么时候问：差得最少的那一个还差多久；都没得
//! 问的，过 `idle` 再问（这一刻还有事的，最早也要再闲 `idle` 才退得下）。
//!
//! 封存了的会话退得下了自己报一声（施工 V-51，`SessionPort::retirable`）：任务当场醒来，只问报了的那几个，问法同上（没有回合、
//! 没人订阅、没有别的把手的才问，actor 照账答）；隔一阵问一遍的照旧按原来的点。
//!
//! 内存紧时（施工 MEM-2，`sessions/pressure.rs`）照同一个问法当场问一遍，只是不等闲够：闲着就退。

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::time::Instant;

use miyu_kernel::id::SessionId;
use miyu_session::{Handle, Retire, Stopped};

use super::{Open, Sessions};
use crate::Core;

/// 闲多久退下：三分钟（`docs/blueprint/perf.md` 量尺的大会话那一项照它等）。
pub(crate) const IDLE: Duration = Duration::from_secs(180);

/// 两次问之间至少隔多久：差几毫秒的不连着问。
const LEAST: Duration = Duration::from_millis(10);

impl Default for Sessions {
    fn default() -> Sessions {
        Sessions::new(IDLE)
    }
}

impl Sessions {
    /// 空的会话表，闲 `idle` 退下。
    fn new(idle: Duration) -> Sessions {
        Sessions {
            open: Mutex::new(Open::default()),
            idle,
            sweeping: AtomicBool::new(false),
            due: Arc::default(),
            answering: std::sync::Mutex::default(),
            pressure: super::pressure::Pressure::default(),
        }
    }

    /// 封存了的会话 `id` 报它退得下了（施工 V-51）：记下、叫醒看空闲的任务。不拿表的锁：可能是在表的锁里放下把手时报的。
    pub(crate) fn retirable(&self, id: SessionId) {
        self.due
            .ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id);
        self.due.rung.notify_one();
    }

    /// 起看空闲的任务（起过的不再起）：只拿着核心的弱引用，核心放下了就停。
    pub(super) fn sweep(&self, core: &Arc<Core>) {
        if self.sweeping.swap(true, Ordering::AcqRel) {
            return;
        }
        // 内存紧时卸闲着的会话（施工 MEM-2）：和看空闲的一起起。
        self.watch_pressure(core);
        let (core, idle, due) = (Arc::downgrade(core), self.idle, Arc::clone(&self.due));
        tokio::spawn(async move {
            let mut at = Instant::now() + idle;
            loop {
                let rung = tokio::select! {
                    () = tokio::time::sleep_until(at) => false,
                    () = due.rung.notified() => true,
                };
                let Some(core) = core.upgrade() else {
                    return;
                };
                if !rung {
                    at = Instant::now() + core.sessions.retire_idle().await.max(LEAST);
                } else if let Some(left) = core.sessions.retire_due().await {
                    at = at.min(Instant::now() + left.max(LEAST));
                }
            }
        });
    }

    /// 问一遍：退下了的、停了的拿掉。交回下一次隔多久再问。
    async fn retire_idle(&self) -> Duration {
        self.retire_all(self.idle).await.next
    }

    /// 拿着表的锁问表里每一个闲够 `idle` 了没有（[`ask`]）：退下了的、停了的拿掉。内存紧时（施工 MEM-2）照 0 问：闲着就退。
    pub(super) async fn retire_all(&self, idle: Duration) -> Swept {
        let mut open = self.open.lock().await;
        let mut next = self.idle;
        let mut gone = Vec::new();
        for (id, running) in &open.running {
            match ask(&running.handle, idle).await {
                Some(Ok(Retire::Retired) | Err(_)) => gone.push(id.clone()),
                Some(Ok(Retire::Later(left))) => next = next.min(left),
                Some(Ok(Retire::Kept)) | None => {}
            }
        }
        for id in &gone {
            open.running.remove(id);
        }
        Swept {
            next,
            gone: gone.len(),
            left: open.running.len(),
        }
    }

    /// 只问报了退得下的那几个（施工 V-51）：退下了的、停了的拿掉；不在表里的（已经退下、停了）不管。没问成的（有回合、有人
    /// 订阅、有别的把手、答了还有事）不再记着：等它们退得下的时候 actor、放下的把手会再报。交回有答了还差多久的，最少的那个。
    async fn retire_due(&self) -> Option<Duration> {
        let due = std::mem::take(&mut *self.due.ids.lock().unwrap_or_else(PoisonError::into_inner));
        if due.is_empty() {
            return None;
        }
        let mut open = self.open.lock().await;
        let mut next: Option<Duration> = None;
        let mut gone = Vec::new();
        for id in due {
            let Some(running) = open.running.get(&id) else {
                continue;
            };
            match ask(&running.handle, self.idle).await {
                Some(Ok(Retire::Retired) | Err(_)) => gone.push(id),
                Some(Ok(Retire::Later(left))) => {
                    next = Some(next.map_or(left, |next| next.min(left)))
                }
                Some(Ok(Retire::Kept)) | None => {}
            }
        }
        for id in gone {
            open.running.remove(&id);
        }
        next
    }
}

/// 问了一遍以后（[`Sessions::retire_all`]）。
pub(super) struct Swept {
    /// 下一次隔多久再问：答了还差多久的取最少的，都没得问的是会话表的闲置时长。
    pub(super) next: Duration,
    /// 拿掉了几个（退下了的、已经停了的）。
    pub(super) gone: usize,
    /// 表里还剩几个。
    pub(super) left: usize,
}

/// 报了退得下的会话（施工 V-51）：哪几个，和叫醒看空闲的任务的那一声（醒之前报了几次都只醒一次，醒着的时候报的留到下一次）。
#[derive(Debug, Default)]
pub(super) struct Due {
    ids: std::sync::Mutex<BTreeSet<SessionId>>,
    rung: Notify,
}

/// 问一个会话退不退：有回合在跑、有人订阅、除了表里这一个还有别的把手的不问（`None`）。
async fn ask(handle: &Handle, idle: Duration) -> Option<Result<Retire, Stopped>> {
    if handle.busy() || handle.watched() || !handle.alone() {
        return None;
    }
    Some(handle.retire(idle).await)
}

impl Core {
    /// 同一份家底，会话闲 `idle` 退下（施工 V-2 再补）：测试里设短的，不用真等三分钟。
    #[must_use]
    pub fn with_session_idle(mut self, idle: Duration) -> Core {
        // 只换时长：别的（测试换上的内存读法，施工 MEM-2）照留。
        self.sessions.idle = idle;
        self
    }

    /// 这时载入着几个会话（施工 V-2 再补）：测试看闲够了的退下了没有。
    pub async fn loaded(&self) -> usize {
        self.sessions.open.lock().await.running.len()
    }
}
