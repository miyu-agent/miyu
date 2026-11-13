//! 内存紧时把闲着的会话卸出内存（施工 MEM-2，`docs/blueprint/protocol.md`「会话表」第 10 条）：隔 [`EVERY`] 看一眼还剩多少
//! 内存（`miyu_heap::room`），开会话、载入会话时也叫它当场看一眼；紧的（[`judge`]），表里闲着的会话当场退下，不等闲够
//! （退下照「会话表」第 9 条的问法：有回合在跑、有人订阅、有别的把手的不问，问了 actor 照自己的账答；记忆抽取的闹钟上着、
//! 还没响的不算还有事，施工 MEM-3）。日志都在盘上，下次用到照常载入。不拒开会话：机器卡死的时候人也得开得了会话救场。
//!
//! 紧的门槛（[`threshold`]，施工 MEM-4）：离总量还有 10%、最多 2 GiB，不低于 5%。离门槛不到两倍的是「近」，近的时候隔
//! [`NEAR_EVERY`] 看一眼：大容器里 5% 判得太晚，判成紧时核心已经顶着上限、交换区满着，卸会话都卸得慢（施工单
//! `MEM-4-大容器提前判紧.md`）。
//!
//! 每次判成紧记一行（施工 MEM-3）：卸了几个、问了几个、没卸的照原因各几个；同一段紧着又没卸掉的，至少隔 [`REPEAT`] 才再记。
//!
//! 为什么是隔一阵看、不等系统通知：Linux 上的 PSI、cgroup 的 `memory.events` 报的是「卡了多久」「撞没撞 `memory.high`」，
//! 不是这里要的「离上限还剩多少、核心自己是大头」，醒了也还得读一遍数；macOS、Windows 各有各的通知，三套接下来都只是换个
//! 时机读同样的数。读一次是 `/proc`、`/sys` 下几个小文件（macOS、Windows 问系统一次），隔 10 秒一次的开销可以不计，每秒
//! 一次只在近线的时候有；表空着的时候不读，只等开会话叫一声。

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;

use miyu_heap::room::{Room, Rooms};

use miyu_session::Retiring;

use super::Sessions;
use crate::Core;

/// 门槛不低于总量的百分之几：5（施工 MEM-1 起的门槛，MEM-4 起是下限：总量很大的，2 GiB 不到 5%，照 5%）。
const FLOOR_PERCENT: u64 = 5;

/// 门槛照总量的百分之几留余量：10，最多 [`SPARE_CAP`]（施工 MEM-4，2026-11-13 core1 定）。
const SPARE_PERCENT: u64 = 10;

/// 余量最多多少：2 GiB。20 GiB 以上的总量，10% 超过 2 GiB，照 2 GiB；40 GiB 以上的，2 GiB 不到 5%，照 5%。
const SPARE_CAP: u64 = 2 * 1024 * 1024 * 1024;

/// 可用的少于门槛的几倍算「近」：2（施工 MEM-4）。近的时候隔 [`NEAR_EVERY`] 看。
const NEAR_TIMES: u64 = 2;

/// 整台机器近线、紧了时，核心自己的常驻占整台机器已用的百分之几以上才算：50。别的程序吃光内存、核心才几百 MB 的时候，卸会话
/// 腾不出多少，还白白丢掉载入着的会话，不算紧；也不为它每秒看一次。正好一半的算。
const MOSTLY_MINE: u64 = 50;

/// 表里有会话、离线还远时隔多久看一眼：10 秒。一个会话常驻约 1.75 MB（2026-11-11 基准测试 M3），10 秒里涨不到撑爆；一下子
/// 涨得快的是一阵开会话，那由开会话时叫的那一声管。
const EVERY: Duration = Duration::from_secs(10);

/// 近线（含紧）时隔多久看一眼：1 秒（施工 MEM-4）。离门槛不到一倍的余量，一批同时跑的会话几十秒就用完。
const NEAR_EVERY: Duration = Duration::from_secs(1);

/// 两次看之间至少隔多久：1 秒。一阵开几百个会话时叫的那几百声并成一次，读数一秒里变不了多少。
const GAP: Duration = Duration::from_secs(1);

/// 同一段紧期间（中间一直判成紧），一个都没卸掉的那一行至少隔多久再记：一分钟（施工 MEM-3）。卸了会话的、一段紧刚开始的
/// 当场记；不限的话一直紧着、没得卸的时候每看一次刷一行一样的。
const REPEAT: Duration = Duration::from_secs(60);

/// 一 MiB，记日志用。
const MIB: u64 = 1024 * 1024;

/// 怎么读还剩多少：出厂是 [`miyu_heap::room::room`]。
type ReadRooms = Arc<dyn Fn() -> Rooms + Send + Sync>;

/// 怎么读核心自己的常驻：出厂是 [`miyu_heap::usage::resident`]（Linux 读 `/proc/self/status` 的 `VmRSS`，便宜）。
type ReadResident = Arc<dyn Fn() -> Option<u64> + Send + Sync>;

/// 看一眼的结果（[`judge`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gauge {
    /// 离线还远：可用的不少于门槛的 [`NEAR_TIMES`] 倍；读不到的、别的程序吃的也算。
    Far,
    /// 近：可用的少于门槛的 [`NEAR_TIMES`] 倍，还不少于门槛。不卸，隔 [`NEAR_EVERY`] 再看。
    Near,
    /// 紧：可用的少于门槛，带着紧的那一份（记日志用）。
    Tight(Room),
}

/// 内存紧时卸会话的这一套：怎么读、隔多久看、开会话时叫的那一声。
pub(super) struct Pressure {
    rooms: ReadRooms,
    resident: ReadResident,
    /// 表里有会话、离线还远时隔多久看一眼：出厂 [`EVERY`]，测试里设短的。
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

    /// 这一刻离线多远：在阻塞线程里读。读的那一下崩了的当不知道，算远。
    async fn look(&self) -> Gauge {
        let (rooms, resident) = (Arc::clone(&self.rooms), Arc::clone(&self.resident));
        tokio::task::spawn_blocking(move || judge(rooms(), || resident()))
            .await
            .unwrap_or(Gauge::Far)
    }

    /// 看了 `gauge` 以后隔多久再看：远的隔 `every`；近的、紧的隔 [`NEAR_EVERY`]，测试里 `every` 比它短的照 `every`。
    fn wait(&self, gauge: Gauge) -> Duration {
        match gauge {
            Gauge::Far => self.every,
            Gauge::Near | Gauge::Tight(_) => self.every.min(NEAR_EVERY),
        }
    }
}

/// 总量 `total` 的门槛（施工 MEM-4）：max(总量的 [`FLOOR_PERCENT`]%, min(总量的 [`SPARE_PERCENT`]%, [`SPARE_CAP`]))，照
/// 整数往下取整。1.5 GiB 是 153.6 MiB，18 GiB 是 1843.2 MiB，20 到 40 GiB 是 2 GiB，100 GiB 是 5 GiB。
fn threshold(total: u64) -> u64 {
    // 乘出来的不比 `total` 大，转得回 u64。
    let share = |percent: u64| {
        u64::try_from(u128::from(total) * u128::from(percent) / 100).unwrap_or(u64::MAX)
    };
    share(FLOOR_PERCENT).max(share(SPARE_PERCENT).min(SPARE_CAP))
}

/// 一份读数离线多远：可用的少于 [`threshold`] 的紧，少于它 [`NEAR_TIMES`] 倍的近，别的远。正好等于的不算少。
fn gauge(room: Room) -> Gauge {
    let line = threshold(room.total);
    if room.available < line {
        Gauge::Tight(room)
    } else if room.available < line.saturating_mul(NEAR_TIMES) {
        Gauge::Near
    } else {
        Gauge::Far
    }
}

/// 离线多远，紧的交回紧的那一份（记日志用）：
///
/// - 核心所在的 cgroup 有上限的，照那一份判（[`gauge`]）。上限是给核心定的，撞上了被杀的是核心，核心占多少都算。
/// - 整台机器的照它判，近了、紧了的还要核心自己的常驻（`resident`，只在这时读）占整台机器已用的 [`MOSTLY_MINE`]% 以上，
///   不是的算远（别的程序吃的）。
///
/// 两份取紧的那一边：一边紧就紧（cgroup 的先），一边近就近。读不到的算远。
pub(super) fn judge(rooms: Rooms, resident: impl FnOnce() -> Option<u64>) -> Gauge {
    let cgroup = rooms.cgroup.map_or(Gauge::Far, gauge);
    if let Gauge::Tight(_) = cgroup {
        return cgroup;
    }
    let system = rooms
        .system
        .map_or(Gauge::Far, |system| match gauge(system) {
            Gauge::Far => Gauge::Far,
            _ if !mostly_mine(system, resident) => Gauge::Far,
            seen => seen,
        });
    match (cgroup, system) {
        (_, Gauge::Tight(_)) => system,
        (Gauge::Near, _) | (_, Gauge::Near) => Gauge::Near,
        _ => Gauge::Far,
    }
}

/// 核心自己的常驻占整台机器已用（总量减可用）的 [`MOSTLY_MINE`]% 以上没有：正好一半的算，读不到的不算。
fn mostly_mine(system: Room, resident: impl FnOnce() -> Option<u64>) -> bool {
    let used = system.total.saturating_sub(system.available);
    resident()
        .is_some_and(|mine| u128::from(mine) * 100 >= u128::from(used) * u128::from(MOSTLY_MINE))
}

impl Sessions {
    /// 表里放进了一个会话（造、载入、子会话、顾问、整理会话）：叫看的任务当场看一眼紧不紧。不读、不等：开会话照常开。
    pub(super) fn opened(&self) {
        self.pressure.rung.notify_one();
    }

    /// 起看内存紧不紧的任务（和看空闲的一起起，只起一次，[`Sessions::sweep`]）：只拿着核心的弱引用，核心放下了就停。表里
    /// 有会话的照上一眼定隔多久再看（[`Pressure::wait`]：远的 `every`，近的 1 秒）；空着的不看，只等开会话叫一声（隔会话表
    /// 的闲置时长醒一下，看核心还在不在）。
    pub(super) fn watch_pressure(&self, core: &Arc<Core>) {
        let (core, rung) = (Arc::downgrade(core), Arc::clone(&self.pressure.rung));
        let (mut wait, gap, idle) = (self.pressure.every, self.pressure.gap, self.idle);
        tokio::spawn(async move {
            let mut said = None;
            loop {
                tokio::select! {
                    () = rung.notified() => {}
                    () = tokio::time::sleep(wait) => {}
                }
                let Some(core) = core.upgrade() else {
                    return;
                };
                wait = core.sessions.relieve(&mut said).await.unwrap_or(idle);
                drop(core);
                tokio::time::sleep(gap).await;
            }
        });
    }

    /// 看一眼：表里有会话、内存紧的，闲着的当场退下（[`Sessions::retire_all`] 照 [`Retiring::Tight`] 问），记一行 `INFO memory
    /// tight`：卸了几个、问了几个、没卸的照原因各几个、表里还剩几个、卸之前读到还剩多少、那一份的门槛。`said` 是这一段紧着
    /// 以来上次记的时刻，不紧的、表空着的放下：卸了的、一段紧刚开始的当场记，一个都没卸的隔 [`REPEAT`] 才再记。表空着的不读。
    /// 交回隔多久再看（[`Pressure::wait`]）；表里没有会话了的交回没有。
    async fn relieve(&self, said: &mut Option<Instant>) -> Option<Duration> {
        if self.open.lock().await.running.is_empty() {
            *said = None;
            return None;
        }
        let gauge = self.pressure.look().await;
        let wait = self.pressure.wait(gauge);
        let Gauge::Tight(room) = gauge else {
            *said = None;
            return Some(wait);
        };
        let swept = self.retire_all(Retiring::Tight).await;
        if swept.gone > 0 {
            // 表里拿掉的那几份（把手、表里记的）是会话退下以后才放的：卸完这一批再攒一次还给系统（施工 MEM-3）。
            miyu_session::trim_soon();
        }
        if swept.gone > 0 || said.is_none_or(|at| at.elapsed() >= REPEAT) {
            tracing::info!(target: "miyu::endpoint", retired = swept.gone, asked = swept.asked, held = %swept.held, loaded = swept.left, available_mib = room.mib(), percent = room.percent(), threshold_mib = threshold(room.total) / MIB, "memory tight");
            *said = Some(Instant::now());
        }
        (swept.left > 0).then_some(wait)
    }
}

/// 测试里换掉读法（施工 MEM-2）。
#[cfg(any(test, feature = "testkit"))]
impl crate::Core {
    /// 同一份家底，还剩多少内存照 `rooms` 读、核心自己的常驻照 `resident` 读，表里有会话、离线还远时隔 `every` 看一眼（近线时
    /// 隔 1 秒，`every` 比 1 秒短的照 `every`），两次之间不歇：测试照它造「核心是大头、只剩 3%」「别的程序吃满」，不真把内存
    /// 吃满。
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
