//! 一个会话的抽取状态（施工 R-6 上，从 `extract.rs` 挪出来）：闹钟是第几个、有没有一次在路上、同一段失败了几次；一直不闲的会话结束了几轮、有没有一次不等闲的在等着（施工 E-5）。

use std::sync::{Mutex, PoisonError};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_kernel::time::Timestamp;

use super::TRIES;

/// 一个会话的抽取：闹钟是第几个、有没有一次在路上、同一段失败了几次；叫停这个会话叫的整理的那一头（施工 R-22）；上次开始抽
/// 以后结束了几轮、最早那一轮什么时候结束的，有没有一次不等闲的在等着（施工 E-5）。
#[derive(Debug, Default)]
pub(crate) struct Extractor {
    state: Mutex<State>,
    pub(crate) hushes: crate::memory::merge::Hushes,
}

#[derive(Debug, Default)]
struct State {
    /// 第几个闹钟：上了新的、撤掉的都往上数，响的时候对不上的是作废的。
    generation: u64,
    /// 有一次在路上。
    running: bool,
    /// 第 `generation` 个闹钟上了，还没响完（施工 V-2 再补）：响了开始抽、作废了、读不了的放下。
    armed: bool,
    /// 从第几条起抽、失败了几次。
    failures: Option<(Seq, u32)>,
    /// 上次开始抽以后结束了几轮（施工 E-5）：落了盘的 `turn.ended` 数，开始抽一次清零。重启以后从零数。
    ended: u32,
    /// 这几轮里最早那一轮结束的时刻。
    first_ended: Option<Timestamp>,
    /// 一次不等闲的（施工 E-5）上了、还没开始：新一轮开始不撤它。
    forcing: bool,
}

impl Extractor {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 上一个新闹钟，交回它是第几个。
    pub(crate) fn arm(&self) -> u64 {
        let mut state = self.state();
        state.generation += 1;
        state.armed = true;
        state.generation
    }

    /// 撤掉闹钟：新一轮开始了。在路上的那一次照样走完。
    pub(crate) fn cancel(&self) {
        let mut state = self.state();
        state.generation += 1;
        state.armed = false;
    }

    /// 第 `generation` 个闹钟响了、不抽（还没闲着、没装记忆、正有一次在路上……）：放下它。对不上的是作废的，不管。
    pub(crate) fn disarm(&self, generation: u64) {
        let mut state = self.state();
        if state.generation == generation {
            state.armed = false;
        }
    }

    /// 有闹钟上着、或者有一次在路上（施工 V-2 再补）、一次不等闲的在等着（施工 E-5）：会话这时不退下。
    pub(crate) fn pending(&self) -> bool {
        let state = self.state();
        state.armed || state.running || state.forcing
    }

    /// 落了盘的这一批 `events`（施工 E-5）：数结束了的回合，记下最早那一轮结束的时刻。
    pub(crate) fn note_ended(&self, events: &[Event]) {
        let mut state = self.state();
        for event in events {
            if matches!(event.body, Body::TurnEnded(_)) {
                state.ended = state.ended.saturating_add(1);
                state.first_ended.get_or_insert(event.at);
            }
        }
    }

    /// 该不等闲就抽了（施工 E-5，`memory.md` 第六条第 1 款）：没有一次在路上、没有一次不等闲的在等着，上次开始抽以后结束了
    /// 至少 `max_turns` 轮，或者最早那一轮结束到 `now` 已经满 `max_wait`。
    pub(crate) fn overdue(
        &self,
        now: Timestamp,
        max_turns: u32,
        max_wait: std::time::Duration,
    ) -> bool {
        let state = self.state();
        if state.running || state.forcing || state.ended == 0 {
            return false;
        }
        let waited = state.first_ended.is_some_and(|first| {
            let ms = now.unix_millis().saturating_sub(first.unix_millis());
            u128::try_from(ms).is_ok_and(|ms| ms >= max_wait.as_millis())
        });
        state.ended >= max_turns || waited
    }

    /// 上一次不等闲的（施工 E-5）：交回上没上成（有一次在路上、已经有一次在等着的不上）。新一轮开始不撤它。
    pub(crate) fn force(&self) -> bool {
        let mut state = self.state();
        if state.running || state.forcing {
            return false;
        }
        state.forcing = true;
        true
    }

    /// 不等闲的那一次不抽了（读不了、没装记忆、已经有一次在路上）：放下它。
    pub(crate) fn unforce(&self) {
        self.state().forcing = false;
    }

    /// 不等闲的那一次开始：交回开没开成（已经有一次在路上的不开）。不看闹钟是第几个：新一轮开始了照样抽，抽到读的那一刻
    /// 结束了的最后一轮。
    pub(crate) fn start_forced(&self) -> bool {
        let mut state = self.state();
        let was = std::mem::take(&mut state.forcing);
        if !was || state.running {
            return false;
        }
        state.running = true;
        state.ended = 0;
        state.first_ended = None;
        true
    }

    /// 有一次在路上。
    pub(crate) fn running(&self) -> bool {
        self.state().running
    }

    /// 第 `generation` 个闹钟还算数、没有一次在路上。
    pub(crate) fn due(&self, generation: u64) -> bool {
        let state = self.state();
        state.generation == generation && !state.running
    }

    /// 第 `generation` 个闹钟还算数就开始一次，交回开没开成。
    pub(crate) fn start(&self, generation: u64) -> bool {
        let mut state = self.state();
        if state.generation != generation || state.running {
            return false;
        }
        state.running = true;
        state.armed = false;
        state.ended = 0;
        state.first_ended = None;
        true
    }

    /// 这一次读不成（日志读不了）：放下，不算这一段失败了一次，下次闲了再来。
    pub(crate) fn abandon(&self) {
        self.state().running = false;
    }

    /// 这一次走完了：成了的清掉失败的次数；没成的交回从第 `after` 条起这是第几次。
    pub(super) fn finish(&self, after: Seq, ok: bool) -> u32 {
        let mut state = self.state();
        state.running = false;
        if ok {
            state.failures = None;
            return 0;
        }
        let tries = match state.failures {
            Some((from, tries)) if from == after => tries + 1,
            _ => 1,
        };
        state.failures = (tries < TRIES).then_some((after, tries));
        tries
    }
}

#[cfg(test)]
mod tests;
