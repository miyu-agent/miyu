//! 读一遍、边读边记（施工 V-39 上）：事件照先后交给真的有效历史，前面谁都碰不到的那一截收下就放掉（怎么放见
//! `crate::sweep`，施工 SP-2 挪出去和问话列表共用）。这里收下的：要判的那一条以前的记录数一数、留最后几条，撤回、引用的
//! 编号记下，要判的那一条拷下来；第一条（钉下的字照它找）。要建记录窗的（施工 V-44，`recent.rs`），同一遍顺手收一份到读到的
//! 最后一条为止的窗。

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use miyu_assemble::group::{backfilled, on_record, quoted_as};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{ExternalId, Seq};

use super::recent::{Caps, Lately};
use super::unseen::{Quoted, judged};
use crate::sweep::bones::hash;
use crate::sweep::{Plan, Settle, Sweep, Tuning};

/// 判官的一遍。
pub(super) type Pass = Sweep<Kept>;

/// 开始一遍：要判第 `msg` 条，最后几条留 `need` 条（[`miyu_assemble::group::Window::most`]）。
#[cfg(test)]
pub(super) fn start(msg: Seq, need: usize, plan: Plan, tuning: Tuning) -> Pass {
    start_building(msg, need, plan, tuning, None)
}

/// 同 `start`（测试用的那一个）；`build` 有的，同一遍照它收一份记录窗（施工 V-44）。
pub(super) fn start_building(
    msg: Seq,
    need: usize,
    plan: Plan,
    tuning: Tuning,
    build: Option<Caps>,
) -> Pass {
    Sweep::new(
        plan,
        tuning,
        Kept {
            msg,
            need,
            first: None,
            judged: None,
            before: 0,
            ring: VecDeque::with_capacity(need + 1),
            recalled: Vec::new(),
            quotes: HashMap::new(),
            unseen: BTreeSet::new(),
            unsure: BTreeSet::new(),
            backfilled: HashSet::new(),
            lately: build.map(Lately::new),
        },
    )
}

/// 一遍读下来收的。
#[derive(Debug)]
pub(super) struct Kept {
    /// 要判的是第几条。
    msg: Seq,
    /// 最后几条留几条。
    need: usize,
    /// 第一条（钉下的字照它找）。
    pub(super) first: Option<Event>,
    /// 要判的那一条，还在有效历史里的话。
    pub(super) judged: Option<Event>,
    /// 它以前的记录一共几条。
    pub(super) before: usize,
    /// 它以前的记录的最后 `need` 条。
    pub(super) ring: VecDeque<Event>,
    /// 它以前的撤回，照先后：平台编号、谁撤的。
    pub(super) recalled: Vec<(String, ExternalId)>,
    /// 它以前引用能指到的编号的哈希（`bones::hash`，不拷编号）→ 最后一条的序号；撞了的用的时候对不上，退回整份读。
    pub(super) quotes: HashMap<u64, Seq>,
    /// 它以前的记录里写引用的内容的（施工 QT-1，`unseen.rs`）：收到那一条的时候照它以前的编号判下来的。
    pub(super) unseen: BTreeSet<Seq>,
    /// 同上，说不准的（引用的编号记在补拉回来的那几条里）：问到的照整份读。
    pub(super) unsure: BTreeSet<Seq>,
    /// 它以前补拉回来的那几条的编号的哈希（施工 QT-1）。
    backfilled: HashSet<u64>,
    /// 顺手收的记录窗（施工 V-44）：定下来的每一截都交给它。
    pub(super) lately: Option<Lately>,
}

impl Settle for Kept {
    fn see(&mut self, event: &Event) {
        if self.first.is_none() {
            self.first = Some(event.clone());
        }
    }

    /// 要判的那一条以前的记录数一数，只把可能留到最后的那几条拷进 `ring`（这一截里最后 `need` 条），撤回、引用的编号记下；
    /// 要判的那一条拷下来。
    fn settle(&mut self, events: &[Event]) {
        if let Some(lately) = &mut self.lately {
            lately.settle(events);
        }
        let upto = events.partition_point(|event| event.seq < self.msg);
        if let Some(judged) = events.get(upto).filter(|event| event.seq == self.msg) {
            self.judged = Some(judged.clone());
        }
        let before = &events[..upto];
        let mut last: Vec<&Event> = Vec::new();
        for event in before.iter().rev() {
            if on_record(event) {
                if last.len() < self.need {
                    last.push(event);
                }
                self.before += 1;
            }
        }
        for event in last.into_iter().rev() {
            self.ring.push_back(event.clone());
            if self.ring.len() > self.need {
                self.ring.pop_front();
            }
        }
        for event in before {
            if let Body::VenueRecalled(recalled) = &event.body {
                self.recalled
                    .push((recalled.msg.clone(), recalled.by.clone()));
            }
            if on_record(event) {
                self.quoting(event);
            }
            if let Some(id) = quoted_as(event) {
                self.quotes.insert(hash(id), event.seq);
                if backfilled(event) {
                    self.backfilled.insert(hash(id));
                }
            }
        }
        // 要判的那一条照它以前的全部判（施工 QT-1）：这一截以前的都收进来了。
        if let Some(judged) = events.get(upto).filter(|event| event.seq == self.msg) {
            self.quoting(judged);
        }
    }
}

impl Kept {
    /// 记下 `event` 的引用的内容写不写（施工 QT-1，`unseen.rs`）：照这时记着的编号，正好是它以前的。
    fn quoting(&mut self, event: &Event) {
        match judged(event, &self.quotes, &self.backfilled, false) {
            Quoted::Write => self.unseen.insert(event.seq),
            Quoted::Unsure => self.unsure.insert(event.seq),
            Quoted::Plain | Quoted::Seen => false,
        };
    }
}
