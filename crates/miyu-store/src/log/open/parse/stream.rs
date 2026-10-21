//! 照先后一块块交出去（施工 V-15）：几个线程照 `chunk` 的分法一起解，交的一方拿到第 `k` 块就交，不等整段解完；
//! 解的线程最多领先交出去的 [`AHEAD`] 块。交出去的事件由收的一方用完就放掉（载入时重放完、压缩以前的那一截），整份
//! 不同时留在内存里：新进程里留着的十几 MB 一页页向系统要，虚拟机上缺页贵（`V-15-重开大会话边读边重放.md`）。
//!
//! 交出去的和一次解完的一样：照先后，读不成的那一行是第几行（从 1 数、从这一段开头数）、为什么，它后面的不交。

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use miyu_kernel::event::Event;

use super::{Lines, PER_THREAD, chunk, cuts, threads};

/// 解的线程最多领先交出去的几块（照线程数乘）。
const AHEAD: usize = 2;

/// 收了一块以后：接着交，还是叫停（施工 V-15）。
pub enum Flow {
    /// 接着交。
    Go,
    /// 不要了：没解的不解，解了的放掉。
    Stop,
}

/// `bytes` 是整行（同 `chunk`），解成事件，一块块照先后交给 `each`。交回这一段读不成的那一行（从 1 数）和为什么；
/// 都读成了、或者 `each` 叫停了的，没有。
pub(in crate::log) fn stream(
    bytes: &[u8],
    mut each: impl FnMut(Vec<Event>) -> Flow,
) -> Option<(usize, String)> {
    let threads = threads(bytes.len());
    let cuts = cuts(bytes, threads * PER_THREAD);
    if threads <= 1 || cuts.len() <= 2 {
        let Lines { events, broken } = chunk(bytes);
        if !events.is_empty() && matches!(each(events), Flow::Stop) {
            return None;
        }
        return broken;
    }
    let pieces: Vec<&[u8]> = cuts
        .windows(2)
        .map(|pair| &bytes[pair[0]..pair[1]])
        .collect();
    let shared = Shared {
        state: Mutex::new(State {
            slots: (0..pieces.len()).map(|_| None).collect(),
            claimed: 0,
            taken: 0,
            stop: false,
        }),
        changed: Condvar::new(),
        ahead: threads * AHEAD,
    };
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| shared.work(&pieces));
        }
        let mut before = 0;
        let mut broken = None;
        for at in 0..pieces.len() {
            let Some(Lines {
                events,
                broken: bad,
            }) = shared.take(at)
            else {
                break;
            };
            let count = events.len();
            if !events.is_empty() && matches!(each(events), Flow::Stop) {
                break;
            }
            // 块里的行号从 1 数：接上前面几块交出去的行数（前面的块都读成了，才走到这一块）。
            if let Some((number, why)) = bad {
                broken = Some((before + number, why));
                break;
            }
            before += count;
        }
        shared.stop();
        broken
    })
}

/// 几个线程共用的。
struct Shared {
    /// 各块、领到了哪、交到了哪。
    state: Mutex<State>,
    /// 解好了一块、交出去了一块、叫停了。
    changed: Condvar,
    /// 最多领先几块。
    ahead: usize,
}

/// 各块解好了没有，领、交到了哪。
struct State {
    /// 第 `k` 块解好了的放这里，交出去时拿走。
    slots: Vec<Option<Lines>>,
    /// 下一个要领的是第几块。
    claimed: usize,
    /// 交出去了几块。
    taken: usize,
    /// 叫停了：不再领。
    stop: bool,
}

impl Shared {
    /// 解的线程：照先后领一块、解、放好，领先太多就等；领完了、叫停了就走。解的时候 panic 了的，先叫停（交的一方不再等它），
    /// panic 照样抛出去。
    fn work(&self, pieces: &[&[u8]]) {
        let _halt = Halt(self);
        loop {
            let at = {
                let mut state = self.lock();
                loop {
                    if state.stop || state.claimed >= pieces.len() {
                        return;
                    }
                    if state.claimed < state.taken + self.ahead {
                        break;
                    }
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                state.claimed += 1;
                state.claimed - 1
            };
            let lines = chunk(pieces[at]);
            let mut state = self.lock();
            if !state.stop {
                state.slots[at] = Some(lines);
            }
            drop(state);
            self.changed.notify_all();
        }
    }

    /// 交的一方：等第 `at` 块解好、拿走。叫停了的没有。
    fn take(&self, at: usize) -> Option<Lines> {
        let mut state = self.lock();
        loop {
            if let Some(lines) = state.slots[at].take() {
                state.taken = at + 1;
                drop(state);
                self.changed.notify_all();
                return Some(lines);
            }
            if state.stop {
                return None;
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// 不再领、解了的放掉。
    fn stop(&self) {
        let mut state = self.lock();
        state.stop = true;
        state.slots.clear();
        drop(state);
        self.changed.notify_all();
    }

    /// 拿锁：别的线程 panic 了也照样拿。
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 解的线程 panic 时叫停。
struct Halt<'a>(&'a Shared);

impl Drop for Halt<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.stop();
        }
    }
}
