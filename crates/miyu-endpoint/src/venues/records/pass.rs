//! 读一遍、边读边记（施工 V-39 上）：事件照先后交给真的有效历史（`History::whole`，撤销、恢复、撤回的规矩一行不另写），
//! 只是它前面那一截谁都碰不到了，就收下来、放掉：要判的那一条以前的记录数一数、留最后几条，撤回、引用的编号记下。有效历史
//! 里只留还可能被撤到的最后一截。
//!
//! 什么时候放：有效历史攒到 [`Tuning::flush_at`] 条、最近一次撤销以后已经开了新的一轮或压缩过（放在一边的撤销清掉了，恢复
//! 不了了）。放到哪：头一遍不知道后面的撤销会撤多早，留最后 [`Tuning::keep`] 条（[`Plan::Guess`]）；读到一次撤销、撤回比
//! 留的还早（骨架算的下界，`bones.rs`），这一遍作废，照头一遍记下的每一次的下界再读一遍，每次只放到后面的撤销都碰不到的地方
//! （[`Plan::Exact`]）。骨架认不得的，调的一方退回整份读。

use std::collections::{HashMap, VecDeque};

use miyu_assemble::group::{on_record, quoted_as};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};
use miyu_store::log::Flow;

use super::bones::{Bones, Unsure, hash};

/// 什么时候放、留多少（测试里调小，逼出放了又撤到的几种）。
#[derive(Debug, Clone, Copy)]
pub(super) struct Tuning {
    /// 有效历史攒到这么多条才放：一条 `Event` 512 字节，2048 条约 1 MB，加上内容约 2 MB。
    pub(super) flush_at: usize,
    /// 头一遍放的时候留最后这么多条（[`Plan::Guess`]）：撤最近一两轮的（主人撤刚才那一句）碰不到更早的。
    pub(super) keep: usize,
}

impl Tuning {
    /// 出厂的。
    pub(super) const SHIPPED: Tuning = Tuning {
        flush_at: 2048,
        keep: 256,
    };
}

/// 放到哪。
#[derive(Debug)]
pub(super) enum Plan {
    /// 头一遍：留最后 [`Tuning::keep`] 条，边读边记骨架。
    Guess(Bones),
    /// 再读一遍：每一次撤销、撤回的序号和下界（头一遍记的，照序号排），`low[k]` 是第 `k` 次及以后的下界里最早的。
    Exact {
        /// 序号、下界。
        reaches: Vec<(Seq, Seq)>,
        /// 后缀的最小值。
        low: Vec<Seq>,
    },
}

impl Plan {
    /// 照头一遍记下的再读一遍。
    pub(super) fn exact(reaches: Vec<(Seq, Seq)>) -> Plan {
        let mut low = vec![Seq::FIRST; reaches.len()];
        let mut least: Option<Seq> = None;
        for (k, &(_, reach)) in reaches.iter().enumerate().rev() {
            let at = least.map_or(reach, |least| least.min(reach));
            least = Some(at);
            low[k] = at;
        }
        Plan::Exact { reaches, low }
    }
}

/// 这一遍怎么样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ended {
    /// 收齐了。
    Done,
    /// 有一次撤销、撤回比放掉的还早：照 [`Pass::reaches`] 再读一遍。
    Again,
    /// 骨架认不得：整份读。
    Unsure,
}

/// 一遍读下来收的。
#[derive(Debug)]
pub(super) struct Pass {
    /// 要判的是第几条。
    msg: Seq,
    /// 最后几条留几条（[`miyu_assemble::group::Window::most`]）。
    need: usize,
    plan: Plan,
    tuning: Tuning,
    /// 有效历史的最后一截：日志里序号不小于 `from` 的、还在有效历史里的都在这里。
    tail: History,
    from: Seq,
    /// 最近一次撤销以后还没开新的一轮、没压缩过：放在一边的撤销还能恢复，不放。
    reverting: bool,
    /// 第一条（钉下的字照它找）。
    pub(super) first: Option<Event>,
    /// 读到的最后一条。
    pub(super) last: Option<Seq>,
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
    /// 每一次撤销、撤回的序号和下界（头一遍记）。
    pub(super) reaches: Vec<(Seq, Seq)>,
    /// 怎么样了。
    pub(super) ended: Ended,
}

impl Pass {
    /// 开始一遍。
    pub(super) fn new(msg: Seq, need: usize, plan: Plan, tuning: Tuning) -> Pass {
        Pass {
            msg,
            need,
            plan,
            tuning,
            tail: History::whole(),
            from: Seq::FIRST,
            reverting: false,
            first: None,
            last: None,
            judged: None,
            before: 0,
            ring: VecDeque::with_capacity(need + 1),
            recalled: Vec::new(),
            quotes: HashMap::new(),
            reaches: Vec::new(),
            ended: Ended::Done,
        }
    }

    /// 读到的一块，照先后。再读的那一遍作废了就叫停；头一遍照旧读完（整份自检、骨架都要读完）。
    pub(super) fn feed(&mut self, events: Vec<Event>) -> Flow {
        for event in events {
            self.one(event);
        }
        match (&self.plan, self.ended) {
            (Plan::Exact { .. }, Ended::Again | Ended::Unsure) => Flow::Stop,
            _ => Flow::Go,
        }
    }

    /// 读完了：有效历史里剩下的都收下。
    pub(super) fn finish(&mut self) {
        if self.ended == Ended::Done {
            let tail = std::mem::take(&mut self.tail);
            self.settle(tail.events());
        }
    }

    fn one(&mut self, event: Event) {
        if self.first.is_none() {
            self.first = Some(event.clone());
        }
        self.last = Some(event.seq);
        let reach = match &mut self.plan {
            Plan::Guess(bones) => {
                let reach = bones.reach(&event);
                bones.note(&event);
                match reach {
                    Ok(reach) => reach,
                    Err(Unsure) => {
                        self.ended = Ended::Unsure;
                        None
                    }
                }
            }
            Plan::Exact { reaches, .. } => reaches
                .binary_search_by_key(&event.seq, |reach| reach.0)
                .ok()
                .map(|k| reaches[k].1),
        };
        if let Some(reach) = reach {
            if matches!(self.plan, Plan::Guess(_)) {
                self.reaches.push((event.seq, reach));
            }
            if reach < self.from && self.ended == Ended::Done {
                self.ended = Ended::Again;
            }
        }
        if self.ended != Ended::Done {
            return;
        }
        match event.body {
            Body::TurnReverted(_) => self.reverting = true,
            Body::TurnStarted(_) | Body::ContextCompacted(_) => self.reverting = false,
            _ => {}
        }
        let at = event.seq;
        self.tail.append(event);
        if !self.reverting && self.tail.events().len() >= self.tuning.flush_at {
            self.flush(at);
        }
    }

    /// 刚交了第 `at` 条：放掉有效历史前面谁都碰不到的那一截。
    fn flush(&mut self, at: Seq) {
        let events = self.tail.events();
        let keep_from = match &self.plan {
            Plan::Guess(_) => events[events.len().saturating_sub(self.tuning.keep.max(1))].seq,
            Plan::Exact { reaches, low } => {
                let k = reaches.partition_point(|reach| reach.0 <= at);
                low.get(k).map_or(at.next(), |&low| low.min(at.next()))
            }
        };
        if keep_from <= self.from {
            return;
        }
        let cut = events.partition_point(|event| event.seq < keep_from);
        if cut > 0 {
            let tail = std::mem::take(&mut self.tail);
            self.settle(&tail.events()[..cut]);
            self.tail = tail.after(tail.events()[cut - 1].seq);
        }
        self.from = keep_from;
    }

    /// 这一截（照先后）在有效历史里定下来了，谁都撤不掉了：要判的那一条以前的记录数一数，只把可能留到最后的那几条拷进
    /// `ring`（这一截里最后 `need` 条），撤回、引用的编号记下；要判的那一条拷下来。
    fn settle(&mut self, events: &[Event]) {
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
            if let Some(id) = quoted_as(event) {
                self.quotes.insert(hash(id), event.seq);
            }
        }
    }
}
