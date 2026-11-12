//! 内存不够不开会话（施工 MEM-1，`docs/blueprint/protocol.md`「会话表」第 10 条）：新造会话、把没在表里的会话从磁盘载入、造子会话
//! 之前看一眼还剩多少可用内存（`miyu_heap::room`），低于 [`LOW_PERCENT`] 的当场拒 `low_memory`。一个会话常驻约 1.75 MB，一直开
//! 新会话会把核心撑到系统的上限、被杀掉，所有会话一起没（2026-11-11 基准测试 M3）；已经在表里的会话照常用。
//!
//! 读不到的（平台不支持、文件不在）当作不知道，照常开。读数留 [`FRESH`]：一阵造很多会话的时候不每次都读。

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use miyu_heap::room::Room;

use super::Sessions;
use crate::refusal::{Locale, Refusal};

/// 可用内存低于总量的百分之几就不开会话：5（2026-11-12 项目主人定，做法 A）。写死，不做配置项。
pub(crate) const LOW_PERCENT: u64 = 5;

/// 一次读数用多久：1 秒。Linux 上一次要读几个小文件（cgroup 每往上一层多三个），连着造一串会话的不每次都读；内存一秒里掉不了多少。
const FRESH: Duration = Duration::from_secs(1);

/// 怎么读还剩多少：出厂是 [`miyu_heap::room::room`]，测试里换成给定的读数。
type Probe = Arc<dyn Fn() -> Option<Room> + Send + Sync>;

/// 开会话之前过的这一道：怎么读，和上一次的读数。
pub(super) struct Gate {
    probe: Probe,
    /// 读数用多久：出厂 [`FRESH`]，测试里是 0（每次都读，改了读数当场生效）。
    fresh: Duration,
    /// 上一次什么时候读的、读到了什么（读不到的是没有）。
    last: Mutex<Option<(Instant, Option<Room>)>>,
}

impl Default for Gate {
    fn default() -> Gate {
        Gate::new(Arc::new(miyu_heap::room::room), FRESH)
    }
}

impl fmt::Debug for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gate")
            .field("fresh", &self.fresh)
            .finish_non_exhaustive()
    }
}

impl Gate {
    fn new(probe: Probe, fresh: Duration) -> Gate {
        Gate {
            probe,
            fresh,
            last: Mutex::new(None),
        }
    }

    /// 这一刻还剩多少：上一次的读数还新鲜的照它，不然在阻塞线程里读一次。读的那一下崩了的当不知道。
    async fn read(&self) -> Option<Room> {
        if let Some((at, room)) = *self.last.lock().unwrap_or_else(PoisonError::into_inner)
            && at.elapsed() < self.fresh
        {
            return room;
        }
        let probe = Arc::clone(&self.probe);
        let room = tokio::task::spawn_blocking(move || probe())
            .await
            .ok()
            .flatten();
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some((Instant::now(), room));
        room
    }
}

impl Sessions {
    /// 开会话之前过这一道：可用内存低于 [`LOW_PERCENT`] 的拒 `low_memory`（带剩多少），记一行 `WARN`；够的、不知道的放过。
    ///
    /// # Errors
    ///
    /// 剩得太少：[`Refusal::low_memory`]。
    pub(crate) async fn room(&self) -> Result<(), Refusal> {
        match self.gate.read().await {
            Some(room) if room.below(LOW_PERCENT) => {
                tracing::warn!(target: "miyu::endpoint", available_mib = room.mib(), percent = room.percent(), "low memory, session not opened");
                Err(Refusal::low_memory(&room))
            }
            _ => Ok(()),
        }
    }

    /// 同 [`Sessions::room`]，给造子会话、顾问会话的几处（施工 MEM-1）：原因是英文的一句，执行器只记进运行日志。
    ///
    /// # Errors
    ///
    /// 剩得太少：拒绝那一句的英文。
    pub(super) async fn room_for_child(&self) -> Result<(), String> {
        self.room()
            .await
            .map_err(|refusal| refusal.message(Locale::En).into_owned())
    }
}

/// 测试里换掉读法（施工 MEM-1）：照 `probe` 读、每次都读，改了读数当场生效。
#[cfg(any(test, feature = "testkit"))]
impl crate::Core {
    /// 同一份家底，还剩多少可用内存照 `probe` 读（交回没有的是不知道）：测试照它造「只剩 3%」，不真把内存吃满。
    #[must_use]
    pub fn with_memory_probe(
        mut self,
        probe: impl Fn() -> Option<Room> + Send + Sync + 'static,
    ) -> crate::Core {
        self.sessions.gate = Gate::new(Arc::new(probe), Duration::ZERO);
        self
    }
}

#[cfg(test)]
mod tests;
