//! 随机输入里在随机的时刻收成槽、要看历史以前从日志读回放回（施工 LW-10 下，`session/shelve.rs`）：看守照会话 actor 的样子，
//! 每送完一条、会话歇着的，照另一串随机数收（三回里一回，不占原来那几串随机数，输入一条不差）；要送的那一条要看历史的，先从
//! 落了盘的日志读回（每一条照日志的一行过一遍）放回，放回的和收起来以前的逐字段比。别的看守的规矩照查，所以收着时直接来
//! 各种输入（说话、撤销、压缩、停下、重启恢复……）结果和不收的一样；另记每一次请求照内核的写法，好和一直不收的那一趟逐字节比
//! （`random.rs` 的 `shelving_changes_nothing`）。
//!
//! 收着时直接送的（群里旁听的话、`events.append` 记的、空闲时的「落了盘」，施工 LW-36）不放回：追加的另放、落了盘并进槽里。
//! 放回以后和收起来以前的那一份接上收着时追加的那几条比；收着时直接送了几条另记一笔，好知道这条路真走到了。

use super::super::naming::{some_appended, some_overheard};
use super::super::rng::Rng;
use super::*;
use crate::history::History;

/// 看守收成槽的账。
pub(in super::super) struct Shelf {
    /// 收不收、什么时候收的随机数；没有的一直不收。
    rng: Option<Rng>,
    /// 收起来以前的有效历史，和那时追加过几条：放回来以后，它接上收着时追加的那几条，和放回来的比。
    before: Option<(History, usize)>,
    /// 收了几次。
    pub(in super::super) shelvings: usize,
    /// 收着、不放回就直接送了几个命令（施工 LW-36：旁听的话、`events.append`）。
    pub(in super::super) quiet: usize,
    /// 每一次请求（主请求、辅助请求、看图）照内核的写法，照先后。
    pub(in super::super) asked: Vec<Vec<u8>>,
    /// 闲着时插几句旁听的话、几条 `events.append` 的随机数（施工 LW-36，[`Watch::overhear`]）；没有的不插。
    overhear: Option<Rng>,
}

thread_local! {
    /// 这个线程上造的看守一直不收（[`never`]）：对照的那一趟用。
    static NEVER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// 这个线程上造的看守闲着时插旁听的话、`events.append`（[`overhearing`]）：收的那一趟、不收的那一趟都插，插得一样。
    static OVERHEAR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 照 `drive` 跑，这期间造的看守闲着时照另一串随机数插旁听的话、`events.append`（施工 LW-36，[`Watch::overhear`]）。只有
/// 收不收的对照用：别的随机测试照原来的输入走，原来走得到的路照样走得到。
pub(in super::super) fn overhearing<T>(drive: impl FnOnce() -> T) -> T {
    OVERHEAR.set(true);
    let driven = drive();
    OVERHEAR.set(false);
    driven
}

/// 照 `drive` 跑一趟，这一趟的看守一直不收：和同一个种子收的那一趟比（`random/shelving.rs`）。
pub(in super::super) fn never(drive: impl FnOnce() -> Watch) -> Watch {
    NEVER.set(true);
    let watch = drive();
    NEVER.set(false);
    watch
}

impl Shelf {
    /// 种子 `seed` 的：照另一串随机数收；[`never`] 里造的一直不收。
    pub(in super::super) fn new(seed: u64) -> Shelf {
        Shelf {
            rng: (!NEVER.get()).then_some(Rng(seed ^ 0x05E1_F5E1)),
            before: None,
            shelvings: 0,
            quiet: 0,
            asked: Vec::new(),
            overhear: OVERHEAR.get().then_some(Rng(seed ^ 0x0A3B_1E36)),
        }
    }
}

impl Watch {
    /// 会话闲着的时候，照另一串随机数（四回里一回）插一到三条：旁听的一句或者 `events.append` 记的一条，每条后面一半的时候
    /// 跟一次「落了盘」（施工 LW-36）。收着的会话不放回、直接收下它们，落了盘的并进槽里；放回以后、请求、日志都要和不收的一样。
    /// 只在 [`overhearing`] 里造的看守插：插不插只看会话闲不闲，收的那一趟、不收的那一趟一样。
    pub(in super::super) fn overhear(&mut self, session: &mut Session, next_id: &mut u64) {
        let Some(rng) = &mut self.shelf.overhear else {
            return;
        };
        if !session.idle() || rng.below(4) != 0 {
            return;
        }
        let picks: Vec<(bool, u64, bool)> = (0..=rng.below(3))
            .map(|_| (rng.below(3) == 0, rng.next(), rng.below(2) == 0))
            .collect();
        for (appended, salt, landed) in picks {
            let input = match appended {
                true => some_appended(salt, next_id),
                false => some_overheard(next_id),
            };
            self.feed(session, input);
            if landed {
                let last = self.last();
                self.feed(session, stored(last));
            }
        }
    }

    /// 把 `input` 送进会话：送以前要看历史、收着的先放回（[`Watch::unshelve_for`]），送完记下请求、照随机数收
    /// （[`Watch::shelve_after`]）。
    pub(super) fn handled(&mut self, session: &mut Session, input: Input) -> Vec<Action> {
        self.unshelve_for(session, &input);
        let actions = session.handle(input);
        self.shelve_after(session, &actions);
        actions
    }

    /// 送 `input` 以前：要看历史、收着的，从落了盘的日志读回放回，和收起来以前的比。
    fn unshelve_for(&mut self, session: &mut Session, input: &Input) {
        let Some(from) = session.shelved() else {
            return;
        };
        if !session.needs_history(input) {
            if matches!(input, Input::Command(_)) {
                self.shelf.quiet += 1;
            }
            return;
        }
        let seed = self.seed;
        let read: Vec<Event> = self
            .log()
            .iter()
            .filter(|event| event.seq >= from)
            .map(|event| Event::from_line(&event.to_line()).expect("日志的一行读得回来"))
            .collect();
        session
            .unshelve(&read)
            .unwrap_or_else(|e| panic!("种子 {seed}：放不回来：{e}"));
        if let Some((mut before, len)) = self.shelf.before.take() {
            for event in &self.events[len..] {
                before.append(event.clone());
            }
            assert!(
                *session.history() == before,
                "种子 {seed}：放回来的有效历史和收起来以前的不一样"
            );
        }
    }

    /// 送完一条：记下交出的请求；歇着的照随机数收。
    fn shelve_after(&mut self, session: &mut Session, actions: &[Action]) {
        for action in actions {
            match action {
                Action::CallModel { request, .. }
                | Action::Aside { request, .. }
                | Action::Describe { request, .. } => {
                    self.shelf.asked.push(request.canonical_bytes());
                }
                _ => {}
            }
        }
        let Some(rng) = &mut self.shelf.rng else {
            return;
        };
        if !session.resting() || session.shelved().is_some() || rng.below(3) != 0 {
            return;
        }
        let before = session.history().clone();
        if session.shelve().is_some() {
            self.shelf.before = Some((before, self.events.len()));
            self.shelf.shelvings += 1;
        }
    }
}
