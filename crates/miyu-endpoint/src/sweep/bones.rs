//! 日志的骨架（施工 V-39 上；施工 SP-2 从判官挪出来，问话列表也用）：边读边记几样小的，读到一次撤销、撤回时算它最早可能碰到第几条（下界）。有效历史
//! （`History::whole`）撤销时查的只有这几样（`miyu-kernel` 的 `history/undo.rs`）：撤的每一轮的事件（`turn` 是它的）；
//! 这一轮 `turn.started` 的触发；上一轮（触发带的回合，或紧挨着这一轮开头结束的那一轮）里排着没听到的人话；触发不带回合的，
//! 同一个命令一起发的人话。撤回（`message.withdrawn`）只碰它列的那几条。恢复只放回撤销拿走的。
//!
//! 下界照宽的算：上一轮两种认法都算上，同一个命令的照命令编号的哈希认（撞了只会更早）。只往前，不往后：算早了多读一点，算晚了
//! 才会错。认不得的（撤的那一轮日志里一条都没有、触发在日志里还没有）交回认不得，调的一方退回整份读。

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{Seq, TurnId};

/// 算不准下界：调的一方退回整份读。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unsure;

/// 读到这里为止的骨架。
#[derive(Debug, Default)]
pub(crate) struct Bones {
    /// 每一轮最早的一条（`turn` 是它的）。
    first: BTreeMap<TurnId, Seq>,
    /// `turn.started`：它的 `turn`、序号、触发。
    starts: Vec<(Option<TurnId>, Seq, Option<Seq>)>,
    /// `message.user`：序号、回合、命令编号的哈希，照序号排。
    said: Vec<(Seq, Option<TurnId>, Option<u64>)>,
    /// `turn.ended`：序号、回合，照序号排。
    ended: Vec<(Seq, Option<TurnId>)>,
    /// 读到的最后一条。
    seen: Option<Seq>,
    /// 上一条带的回合（同一轮接连几条只查一次 `first`）。
    last_turn: Option<TurnId>,
}

impl Bones {
    /// 记下一条（照先后）。
    pub(crate) fn note(&mut self, event: &Event) {
        self.seen = Some(event.seq);
        if let Some(turn) = event.turn.filter(|&turn| self.last_turn != Some(turn)) {
            self.first.entry(turn).or_insert(event.seq);
            self.last_turn = Some(turn);
        }
        match &event.body {
            Body::TurnStarted(started) => {
                self.starts.push((event.turn, event.seq, started.trigger));
            }
            Body::MessageUser(_) => {
                let cause = event.cause.as_ref().map(|cause| hash(cause.as_str()));
                self.said.push((event.seq, event.turn, cause));
            }
            Body::TurnEnded(_) => self.ended.push((event.seq, event.turn)),
            _ => {}
        }
    }

    /// 这一条（照先后读到它之前已经 [`Bones::note`] 过前面的）是撤销、撤回的：它最早可能碰到、查到第几条；碰不到什么的
    /// （撤的是空的、列的是空的）没有。别的种类没有。
    pub(crate) fn reach(&self, event: &Event) -> Result<Option<Seq>, Unsure> {
        match &event.body {
            Body::TurnReverted(reverted) => {
                let mut low = None;
                for turn in &reverted.turns {
                    let first = *self.first.get(turn).ok_or(Unsure)?;
                    lower(&mut low, first);
                    for &(_, started, trigger) in
                        self.starts.iter().filter(|start| start.0 == Some(*turn))
                    {
                        if let Some(trigger) = trigger {
                            self.trigger(trigger, &mut low)?;
                        }
                        self.ended_before(started, &mut low);
                    }
                }
                Ok(low)
            }
            Body::MessageWithdrawn(withdrawn) => Ok(withdrawn.messages.iter().min().copied()),
            _ => Ok(None),
        }
    }

    /// 触发那一条，和照它认的上一轮、同一个命令一起发的人话。
    fn trigger(&self, trigger: Seq, low: &mut Option<Seq>) -> Result<(), Unsure> {
        if self.seen.is_none_or(|seen| trigger > seen) {
            return Err(Unsure);
        }
        lower(low, trigger);
        let Ok(k) = self.said.binary_search_by_key(&trigger, |said| said.0) else {
            return Ok(());
        };
        match self.said[k] {
            (_, Some(turn), _) => {
                if let Some(&first) = self.first.get(&turn) {
                    lower(low, first);
                }
            }
            (_, None, Some(cause)) => {
                if let Some(&(seq, ..)) = self
                    .said
                    .iter()
                    .find(|said| said.1.is_none() && said.2 == Some(cause))
                {
                    lower(low, seq);
                }
            }
            (_, None, None) => {}
        }
        Ok(())
    }

    /// 紧挨着第 `started` 条（一轮的开头）结束的那一轮：它最早的一条。
    fn ended_before(&self, started: Seq, low: &mut Option<Seq>) {
        let Some(before) = started.get().checked_sub(1).and_then(Seq::new) else {
            return;
        };
        if let Ok(k) = self.ended.binary_search_by_key(&before, |ended| ended.0)
            && let Some(turn) = self.ended[k].1
            && let Some(&first) = self.first.get(&turn)
        {
            lower(low, first);
        }
    }
}

/// 往前挪到 `seq`。
fn lower(low: &mut Option<Seq>, seq: Seq) {
    *low = Some(low.map_or(seq, |low| low.min(seq)));
}

/// 字的哈希：命令编号的只用来认同一个命令，撞了只会把下界算早；引用的编号的（判官的 `venues/records/pass.rs`）用的时候再对一遍。
pub(crate) fn hash(cause: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    cause.hash(&mut hasher);
    hasher.finish()
}
