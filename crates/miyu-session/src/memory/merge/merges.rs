//! 一间一把锁、同一批连着失败几次（施工 R-7 上；R-17 从 `merge.rs` 挪出来）、叫停这一间人叫的整理的那一头（施工 R-23）。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use miyu_store::recall::Room;

use super::super::Memory;
use super::TRIES;
use super::hush::{Hush, Hushes};

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
    /// 每一间叫停人叫的整理的那一头（施工 R-23）：用到哪一间才有。
    hushes: BTreeMap<Room, Hushes>,
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

    /// 开始合 `room`，交回拿着这一间的那一头：放下就算合完了（施工 R-30），半路被丢下、panic 了也放。已经有一个在合的没有。
    pub(super) fn begin(memory: &Arc<Memory>, room: &Room) -> Option<Running> {
        memory.merges.start(room).then(|| Running {
            memory: Arc::clone(memory),
            room: room.clone(),
        })
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

    /// 人叫的整理 `hush` 再听 `room` 这一间的叫停（施工 R-23）。
    pub(super) fn listen(&self, room: &Room, hush: Hush) -> Hush {
        hush.and_room(self.state().hushes.entry(room.clone()).or_default())
    }

    /// 叫停 `room` 这一间这时人叫的整理（施工 R-23），交回有没有。
    pub(super) fn hush(&self, room: &Room) -> bool {
        self.state().hushes.get(room).is_some_and(Hushes::hush)
    }
}

/// 拿着一间合并的锁的那一头（施工 R-30）：放下就放锁。
#[derive(Debug)]
pub(crate) struct Running {
    memory: Arc<Memory>,
    room: Room,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.memory.merges.finish(&self.room);
    }
}
