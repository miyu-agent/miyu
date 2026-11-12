//! 内存紧时把闲着的会话卸出内存（施工 MEM-2，`docs/blueprint/protocol.md`「会话表」第 10 条）：隔 [`EVERY`] 看一眼还剩多少
//! 内存（`miyu_heap::room`），开会话、载入会话时也叫它当场看一眼；紧的（[`tight`]），表里闲着的会话当场退下，不等闲够
//! （退下照「会话表」第 9 条的问法：有回合在跑、有人订阅、有别的把手的不问，问了 actor 照自己的账答）。日志都在盘上，
//! 下次用到照常载入。不拒开会话：机器卡死的时候人也得开得了会话救场。
//!
//! 为什么是隔一阵看、不等系统通知：Linux 上的 PSI、cgroup 的 `memory.events` 报的是「卡了多久」「撞没撞 `memory.high`」，
//! 不是这里要的「剩不到 5%、核心自己是大头」，醒了也还得读一遍数；macOS、Windows 各有各的通知，三套接下来都只是换个时机
//! 读同样的数。读一次是 `/proc`、`/sys` 下几个小文件（macOS、Windows 问系统一次），隔 10 秒一次的开销可以不计；表空着的
//! 时候不读，只等开会话叫一声。

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use miyu_heap::room::{Room, Rooms};

use super::Sessions;
use crate::Core;

/// 可用内存低于总量的百分之几算紧：5（照施工 MEM-1 的门槛，2026-11-12 项目主人定改成卸会话）。写死，不做配置项。
pub(crate) const LOW_PERCENT: u64 = 5;

/// 整台机器剩不到 [`LOW_PERCENT`] 时，核心自己的常驻占整台机器已用的百分之几以上才算紧：50。别的程序吃光内存、核心才几百 MB
/// 的时候，卸会话腾不出多少，还白白丢掉载入着的会话，不算紧。正好一半的算。
const MOSTLY_MINE: u64 = 50;

/// 表里有会话时隔多久看一眼：10 秒。一个会话常驻约 1.75 MB（2026-11-11 基准测试 M3），10 秒里涨不到撑爆；一下子涨得快的是
/// 一阵开会话，那由开会话时叫的那一声管。
const EVERY: Duration = Duration::from_secs(10);

/// 两次看之间至少隔多久：1 秒。一阵开几百个会话时叫的那几百声并成一次，读数一秒里变不了多少。
const GAP: Duration = Duration::from_secs(1);

/// 怎么读还剩多少：出厂是 [`miyu_heap::room::room`]。
type ReadRooms = Arc<dyn Fn() -> Rooms + Send + Sync>;

/// 怎么读核心自己的常驻：出厂是 [`miyu_heap::usage::resident`]（Linux 读 `/proc/self/status` 的 `VmRSS`，便宜）。
type ReadResident = Arc<dyn Fn() -> Option<u64> + Send + Sync>;

/// 内存紧时卸会话的这一套：怎么读、隔多久看、开会话时叫的那一声。
pub(super) struct Pressure {
    rooms: ReadRooms,
    resident: ReadResident,
    /// 表里有会话时隔多久看一眼：出厂 [`EVERY`]，测试里设短的。
    every: Duration,
    /// 两次看之间至少隔多久：出厂 [`GAP`]，测试里是 0。
    gap: Duration,
    /// 开会话、载入会话时叫一声：看的任务当场看一眼。醒之前叫了几次都只看一次。
    rung: Arc<Notify>,
}

impl Default for Pressure {
    fn default() -> Pressure {
        Pressure::new(
            Arc::new(miyu_heap::room::room),
            Arc::new(miyu_heap::usage::resident),
            EVERY,
            GAP,
        )
    }
}

impl fmt::Debug for Pressure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pressure")
            .field("every", &self.every)
            .field("gap", &self.gap)
            .finish_non_exhaustive()
    }
}

impl Pressure {
    fn new(rooms: ReadRooms, resident: ReadResident, every: Duration, gap: Duration) -> Pressure {
        Pressure {
            rooms,
            resident,
            every,
            gap,
            rung: Arc::new(Notify::new()),
        }
    }

    /// 这一刻紧不紧：在阻塞线程里读，紧的交回紧的那一份。读的那一下崩了的当不知道，不算紧。
    async fn tight(&self) -> Option<Room> {
        let (rooms, resident) = (Arc::clone(&self.rooms), Arc::clone(&self.resident));
        tokio::task::spawn_blocking(move || tight(rooms(), || resident()))
            .await
            .ok()
            .flatten()
    }
}

/// 内存紧不紧，紧的交回紧的那一份（记日志用）：
///
/// - 核心所在的 cgroup 有上限、可用低于 [`LOW_PERCENT`]：紧。上限是给核心定的，撞上了被杀的是核心。
/// - 整台机器可用低于 [`LOW_PERCENT`]，而且核心自己的常驻（`resident`，只在这时读）占整台机器已用的 [`MOSTLY_MINE`]% 以上：紧。
///
/// 别的都不算紧：整台机器紧、核心占得少的（别的程序吃的），读不到的（不知道），正好 5% 的。
pub(super) fn tight(rooms: Rooms, resident: impl FnOnce() -> Option<u64>) -> Option<Room> {
    if let Some(cgroup) = rooms.cgroup
        && cgroup.below(LOW_PERCENT)
    {
        return Some(cgroup);
    }
    let system = rooms.system.filter(|system| system.below(LOW_PERCENT))?;
    let used = system.total.saturating_sub(system.available);
    let mine = resident()?;
    (u128::from(mine) * 100 >= u128::from(used) * u128::from(MOSTLY_MINE)).then_some(system)
}

impl Sessions {
    /// 表里放进了一个会话（造、载入、子会话、顾问、整理会话）：叫看的任务当场看一眼紧不紧。不读、不等：开会话照常开。
    pub(super) fn opened(&self) {
        self.pressure.rung.notify_one();
    }

    /// 起看内存紧不紧的任务（和看空闲的一起起，只起一次，[`Sessions::sweep`]）：只拿着核心的弱引用，核心放下了就停。表里
    /// 有会话的隔 `every` 看一眼；空着的不看，只等开会话叫一声（隔会话表的闲置时长醒一下，看核心还在不在）。
    pub(super) fn watch_pressure(&self, core: &Arc<Core>) {
        let (core, rung) = (Arc::downgrade(core), Arc::clone(&self.pressure.rung));
        let (every, gap, idle) = (self.pressure.every, self.pressure.gap, self.idle);
        tokio::spawn(async move {
            let mut loaded = true;
            loop {
                let wait = if loaded { every } else { idle };
                tokio::select! {
                    () = rung.notified() => {}
                    () = tokio::time::sleep(wait) => {}
                }
                let Some(core) = core.upgrade() else {
                    return;
                };
                loaded = core.sessions.relieve().await;
                drop(core);
                tokio::time::sleep(gap).await;
            }
        });
    }

    /// 看一眼：表里有会话、内存紧的，闲着的当场退下（[`Sessions::retire_all`] 照 0 问），卸了的记一行 `INFO`（卸了几个、表里
    /// 还剩几个、卸之前读到还剩多少）。表空着的不读。交回表里还有没有会话。
    async fn relieve(&self) -> bool {
        if self.open.lock().await.running.is_empty() {
            return false;
        }
        let Some(room) = self.pressure.tight().await else {
            return true;
        };
        let swept = self.retire_all(Duration::ZERO).await;
        if swept.gone > 0 {
            tracing::info!(target: "miyu::endpoint", retired = swept.gone, loaded = swept.left, available_mib = room.mib(), percent = room.percent(), "memory tight, idle sessions retired");
        }
        swept.left > 0
    }
}

/// 测试里换掉读法（施工 MEM-2）。
#[cfg(any(test, feature = "testkit"))]
impl crate::Core {
    /// 同一份家底，还剩多少内存照 `rooms` 读、核心自己的常驻照 `resident` 读，表里有会话时隔 `every` 看一眼，两次之间不歇：
    /// 测试照它造「核心是大头、只剩 3%」「别的程序吃满」，不真把内存吃满。
    #[must_use]
    pub fn with_memory_probe(
        mut self,
        every: Duration,
        rooms: impl Fn() -> Rooms + Send + Sync + 'static,
        resident: impl Fn() -> Option<u64> + Send + Sync + 'static,
    ) -> crate::Core {
        self.sessions.pressure =
            Pressure::new(Arc::new(rooms), Arc::new(resident), every, Duration::ZERO);
        self
    }
}

#[cfg(test)]
mod tests;
