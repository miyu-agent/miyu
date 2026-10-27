//! 一个会话的抽取状态（施工 R-6 上，从 `extract.rs` 挪出来）：闹钟是第几个、有没有一次在路上、同一段失败了几次。

use std::sync::{Mutex, PoisonError};

use miyu_kernel::id::Seq;

use super::TRIES;

/// 一个会话的抽取：闹钟是第几个、有没有一次在路上、同一段失败了几次；叫停这个会话叫的整理的那一头（施工 R-22）。
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

    /// 有闹钟上着、或者有一次在路上（施工 V-2 再补）：会话这时退下，这一段要等下次载入、闲下来才抽。
    pub(crate) fn pending(&self) -> bool {
        let state = self.state();
        state.armed || state.running
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
