//! 随机输入里在随机的时刻收成槽、要看历史以前从日志读回放回（施工 LW-10 下，`session/shelve.rs`）：看守照会话 actor 的样子，
//! 每送完一条、会话歇着的，照另一串随机数收（三回里一回，不占原来那几串随机数，输入一条不差）；要送的那一条要看历史的，先从
//! 落了盘的日志读回（每一条照日志的一行过一遍）放回，放回的和收起来以前的逐字段比。别的看守的规矩照查，所以收着时直接来
//! 各种输入（说话、撤销、压缩、停下、重启恢复……）结果和不收的一样；另记每一次请求照内核的写法，好和一直不收的那一趟逐字节比
//! （`random.rs` 的 `shelving_changes_nothing`）。

use super::super::rng::Rng;
use super::*;
use crate::history::History;

/// 看守收成槽的账。
pub(in super::super) struct Shelf {
    /// 收不收、什么时候收的随机数；没有的一直不收。
    rng: Option<Rng>,
    /// 收起来以前的有效历史：放回来以后和它比。
    before: Option<History>,
    /// 收了几次。
    pub(in super::super) shelvings: usize,
    /// 每一次请求（主请求、辅助请求、看图）照内核的写法，照先后。
    pub(in super::super) asked: Vec<Vec<u8>>,
}

thread_local! {
    /// 这个线程上造的看守一直不收（[`never`]）：对照的那一趟用。
    static NEVER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
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
            asked: Vec::new(),
        }
    }
}

impl Watch {
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
        if !Session::needs_history(input) {
            return;
        }
        let Some(from) = session.shelved() else {
            return;
        };
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
        if let Some(before) = self.shelf.before.take() {
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
            self.shelf.before = Some(before);
            self.shelf.shelvings += 1;
        }
    }
}
