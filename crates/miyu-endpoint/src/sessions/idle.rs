//! 闲够了的会话退下（`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条，施工 V-2 再补）：会话表第一次放进会话时起一个
//! 任务，隔一阵拿着表的锁问一遍。没有别的把手、没人订阅、没有在跑的回合的，问它的 actor 闲够了没有（`Handle::retire`，它照
//! 自己的账答）；退下了的、已经停了的从表里拿掉，下次用到再载入。actor 退出时把放下的内存还给系统（`miyu-heap`）。
//!
//! 拿着锁问：这一刻谁也拿不到新的把手，退下和再载入不会同时有两个写者。一批批问（施工 MEM-5）：每批至多 [`BATCH`] 个一起问，
//! 问完一批放一次锁，别的请求插得进来；一万个会话一直拿着锁问，被挤满的机器上要几十秒。下一次什么时候问：差得最少的那一个
//! 还差多久；都没得问的，过 `idle` 再问（这一刻还有事的，最早也要再闲 `idle` 才退得下）。
//!
//! 封存了的会话退得下了自己报一声（施工 V-51，`SessionPort::retirable`）：任务当场醒来，只问报了的那几个，问法同上（没有回合、
//! 没人订阅、没有别的把手的才问，actor 照账答）；隔一阵问一遍的照旧按原来的点。
//!
//! 内存紧时（施工 MEM-2，`sessions/pressure.rs`）照同一个问法当场问一遍，只是不等闲够：闲着就退（[`Retiring::Tight`]：记忆抽取
//! 的闹钟上着、还没响的也退，施工 MEM-3）。没退下的照原因各记几个（[`Held`]），内存紧时记进日志。

use std::collections::BTreeSet;
use std::ops::Bound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use tokio::sync::{Mutex, Notify};
use tokio::time::Instant;

use miyu_kernel::id::SessionId;
use miyu_session::{Handle, Hold, Retire, Retiring};

use super::held::Held;
use super::{Open, Sessions};
use crate::Core;

/// 闲多久退下：三分钟（`docs/blueprint/perf.md` 量尺的大会话那一项照它等）。
pub(crate) const IDLE: Duration = Duration::from_secs(180);

/// 两次问之间至少隔多久：差几毫秒的不连着问。
const LEAST: Duration = Duration::from_millis(10);

/// 一批最多问几个（施工 MEM-5）：问完一批放一次表的锁。一批里的 actor 同时答，一批拿着锁的时间约是问一个的一两倍，和批多大
/// 关系不大；批小了放锁、挑会话的次数多，批大了一次送进运行队列的问话多。64 个在挤满的机器上一批几十毫秒（施工单
/// `MEM-5-卸会话不长拿锁.md`「量」）。
const BATCH: usize = 64;

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
        self.retire_all(Retiring::Idle(self.idle)).await.next
    }

    /// 照 `asking` 问表里每一个（会话表这一头看得出不退的不问，[`skip`]）：退下了的、停了的拿掉，没退下的照原因计数。闲够了
    /// 退下的那一路问闲够会话表的闲置时长没有；内存紧时（施工 MEM-2、MEM-3）照 [`Retiring::Tight`] 问：没事就退。
    ///
    /// 一批批问（施工 MEM-5）：拿着表的锁照编号往后挑出至多 [`BATCH`] 个要问的，一起问（[`Handle::retire_each`]，一批里的
    /// actor 同时答），拿掉退下了的，放开锁；下一批从上一批挑到的地方接着挑。每一批照旧拿着锁问、拿着锁拿掉（这一刻谁也拿不到
    /// 新的把手，[`skip`] 看的「没有别的把手」在问的时候还成立），批与批之间别的请求（开会话、说话、列会话）拿得到锁。这一遍
    /// 中途放进表里的、编号排在后面的也问；排在前面的等下一遍。
    pub(super) async fn retire_all(&self, asking: Retiring) -> Swept {
        let started = Instant::now();
        let mut swept = Swept {
            next: self.idle,
            gone: 0,
            asked: 0,
            held: Held::default(),
            left: 0,
            took: Duration::ZERO,
        };
        let mut after: Option<SessionId> = None;
        loop {
            let mut open = self.open.lock().await;
            let from = after.as_ref().map_or(Bound::Unbounded, Bound::Excluded);
            let (mut ids, mut handles) = (Vec::with_capacity(BATCH), Vec::with_capacity(BATCH));
            let mut last = None;
            for (id, running) in open.running.range::<SessionId, _>((from, Bound::Unbounded)) {
                last = Some(id);
                if let Some(hold) = skip(&running.handle) {
                    swept.held.add(hold);
                } else {
                    ids.push(id);
                    handles.push(&running.handle);
                    if ids.len() == BATCH {
                        break;
                    }
                }
            }
            let answers = Handle::retire_each(&handles, asking).await;
            swept.asked += ids.len();
            let mut gone = Vec::new();
            for (id, answer) in ids.iter().zip(answers) {
                match answer {
                    Ok(Retire::Retired) | Err(_) => gone.push((*id).clone()),
                    Ok(Retire::Later(left)) => swept.next = swept.next.min(left),
                    Ok(Retire::Kept(hold)) => swept.held.add(hold),
                }
            }
            // 挑满了一批的，下一批从这里接着挑；没挑满的是到了表尾。
            after = last.filter(|_| ids.len() == BATCH).cloned();
            for id in &gone {
                open.running.remove(id);
            }
            swept.gone += gone.len();
            swept.left = open.running.len();
            drop(open);
            if after.is_none() {
                break;
            }
        }
        swept.took = started.elapsed();
        swept
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
            if skip(&running.handle).is_some() {
                continue;
            }
            match running.handle.retire(self.idle).await {
                Ok(Retire::Retired) | Err(_) => gone.push(id),
                Ok(Retire::Later(left)) => next = Some(next.map_or(left, |next| next.min(left))),
                Ok(Retire::Kept(_)) => {}
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
    /// 问了 actor 的几个（会话表这一头就看得出不退的不问，见 [`skip`]）。
    pub(super) asked: usize,
    /// 没退下的照原因各几个：会话表这一头看出来的、actor 答的都算。
    pub(super) held: Held,
    /// 表里还剩几个。
    pub(super) left: usize,
    /// 问这一遍花了多久：头一批拿锁起到最后一批放锁（施工 MEM-5，内存紧时记进日志）。
    pub(super) took: Duration,
}

/// 报了退得下的会话（施工 V-51）：哪几个，和叫醒看空闲的任务的那一声（醒之前报了几次都只醒一次，醒着的时候报的留到下一次）。
#[derive(Debug, Default)]
pub(super) struct Due {
    ids: std::sync::Mutex<BTreeSet<SessionId>>,
    rung: Notify,
}

/// 会话表这一头就看得出不退、不用问 actor 的：有回合在跑、有人订阅、除了表里这一个还有别的把手，交回是哪一样。
fn skip(handle: &Handle) -> Option<Hold> {
    if handle.busy() {
        Some(Hold::Turn)
    } else if handle.watched() {
        Some(Hold::Watched)
    } else if !handle.alone() {
        Some(Hold::Handles)
    } else {
        None
    }
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
