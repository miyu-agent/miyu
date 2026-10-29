//! 写不进下一次的先放一放（施工 J-1 上，2026-10-26 主会话审出来的）：到点的一个，下一次写不进表（盘满、只读、锁住），表里
//! 还是过去那个时刻，不放的话睡着的那个马上又醒、又挑出它、又写不进，空转吃满一个核。只在内存里记一个「不早于」：头一次
//! 1 秒，再写不进翻倍，最多 5 分钟；写进了就清掉。不碰表，核心重启就忘（起来时照常跳过错过的）。

use std::collections::BTreeMap;
use std::sync::Mutex;

use miyu_kernel::time::Timestamp;

/// 头一次放多久（毫秒）。
const FIRST: i64 = 1_000;
/// 最多放多久（毫秒）。
const MOST: i64 = 300_000;

/// 放着的几个：任务编号到「不早于」和这一回放了多久。
#[derive(Default)]
pub(super) struct Holds(Mutex<BTreeMap<String, (Timestamp, i64)>>);

impl Holds {
    /// 又写不进一次：照上一回翻倍（头一次 1 秒，最多 5 分钟）往后放，交回不早于哪一刻。
    pub(super) fn failed(&self, id: &str, now: Timestamp) -> Timestamp {
        let mut holds = self.lock();
        let key = id.to_string();
        let wait = holds
            .get(&key)
            .map_or(FIRST, |(_, wait)| wait.saturating_mul(2).min(MOST));
        let until =
            Timestamp::from_unix_millis(now.unix_millis().saturating_add(wait)).unwrap_or(now);
        holds.insert(key, (until, wait));
        until
    }

    /// 写进了：不再放。
    pub(super) fn cleared(&self, id: &str) {
        self.lock().remove(id);
    }

    /// 这一个的下一次 `next` 照放着的往后挪：没放着的照旧。
    pub(super) fn later(&self, id: &str, next: Timestamp) -> Timestamp {
        match self.lock().get(id) {
            Some((until, _)) => next.max(*until),
            None => next,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (Timestamp, i64)>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
