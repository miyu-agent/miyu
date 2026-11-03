//! 读一遍、边读边记（施工 V-39 上）：事件照先后交给真的有效历史，前面谁都碰不到的那一截收下就放掉（怎么放见
//! `crate::sweep`，施工 SP-2 挪出去和问话列表共用）。这里收下的：要判的那一条以前的记录数一数、留最后几条，撤回、引用的
//! 编号记下，要判的那一条拷下来；第一条（钉下的字照它找）。

use std::collections::{HashMap, VecDeque};

use miyu_assemble::group::{on_record, quoted_as};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{ExternalId, Seq};

use crate::sweep::bones::hash;
use crate::sweep::{Plan, Settle, Sweep, Tuning};

/// 判官的一遍。
pub(super) type Pass = Sweep<Kept>;

/// 开始一遍：要判第 `msg` 条，最后几条留 `need` 条（[`miyu_assemble::group::Window::most`]）。
pub(super) fn start(msg: Seq, need: usize, plan: Plan, tuning: Tuning) -> Pass {
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
