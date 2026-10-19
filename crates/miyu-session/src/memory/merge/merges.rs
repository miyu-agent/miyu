//! 一间一把锁、同一批连着失败几次（施工 R-7 上；R-17 从 `merge.rs` 挪出来）。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, PoisonError};

use miyu_store::recall::Room;

use super::TRIES;

/// 核心一份的合并状态：哪几间在合、每一间同一批失败了几次。
#[derive(Debug, Default)]
pub(crate) struct Merges {
    state: Mutex<MergeState>,
}

#[derive(Debug, Default)]
struct MergeState {
    running: BTreeSet<Room>,
    /// 每一间连着失败了几次。
    failures: BTreeMap<Room, u32>,
}

impl Merges {
    fn state(&self) -> std::sync::MutexGuard<'_, MergeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 开始合 `room`：已经有一个在合的交回 `false`。
    pub(super) fn start(&self, room: &Room) -> bool {
        self.state().running.insert(room.clone())
    }

    /// 合完了（成没成都算）。
    pub(super) fn finish(&self, room: &Room) {
        self.state().running.remove(room);
    }

    /// 这一次成没成：成了的清掉失败的次数，交回 0；没成的交回连着第几次，到了 [`TRIES`] 次的也清掉（这一批跳过了）。
    pub(super) fn record(&self, room: &Room, ok: bool) -> u32 {
        let mut state = self.state();
        if ok {
            state.failures.remove(room);
            return 0;
        }
        let tries = state.failures.get(room).copied().unwrap_or(0) + 1;
        if tries >= TRIES {
            state.failures.remove(room);
        } else {
            state.failures.insert(room.clone(), tries);
        }
        tries
    }
}
