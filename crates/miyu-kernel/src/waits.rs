//! 等人不算用时（施工 E-M 下，2026-10-23 项目主人定「统一」）：她在等人批准、等人回答的那几段从用时里刨掉，各处一个口径。
//! 视图的一步、一段、一轮的用时（施工 9-8 十四补），给她看的已用时间（`session_usage`）都照这里算。只认日志里的事件，不看钟：
//! 照一样的事件算出一样的数。
//!
//! 起：`tool.approval_requested`、`question.asked`；止：`tool.approval_decided`、`question.answered`、`tool.result`（被打断、
//! 被停的调用也有结果）。几件一起等的，第一件起、最后一件了结才止，算一段。回合开始、结束都清掉。

use crate::event::{Body, Event};
use crate::id::CallId;
use crate::time::Timestamp;

#[cfg(test)]
mod tests;

/// 一轮里等人的那几段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Waits {
    /// 正在等人的调用。
    pending: Vec<CallId>,
    /// 等人的几段：起、止；还在等的那一段没有止。
    spans: Vec<(Timestamp, Option<Timestamp>)>,
}

impl Waits {
    /// 看一条事件：起、止等人的照它记，回合开始、结束的清掉。交回认没认它。
    pub fn note(&mut self, event: &Event) -> bool {
        match &event.body {
            Body::ApprovalRequested(asked) => self.wait(&asked.call_id, event.at),
            Body::QuestionAsked(asked) => self.wait(&asked.call_id, event.at),
            Body::ApprovalDecided(decided) => self.settled(&decided.call_id, event.at),
            Body::QuestionAnswered(answered) => self.settled(&answered.call_id, event.at),
            Body::ToolResult(result) => self.settled(&result.call_id, event.at),
            Body::TurnStarted(_) | Body::TurnEnded(_) => self.clear(),
            _ => return false,
        }
        true
    }

    /// `call` 在 `at` 开始等人：原来没在等的，起一段。
    pub fn wait(&mut self, call: &CallId, at: Timestamp) {
        if self.pending.contains(call) {
            return;
        }
        if self.pending.is_empty() {
            self.spans.push((at, None));
        }
        self.pending.push(*call);
    }

    /// `call` 在 `at` 了结了：全了结了，止住这一段。
    pub fn settled(&mut self, call: &CallId, at: Timestamp) {
        let before = self.pending.len();
        self.pending.retain(|pending| pending != call);
        if before > 0
            && self.pending.is_empty()
            && let Some((_, end)) = self.spans.last_mut()
        {
            *end = Some(at);
        }
    }

    /// 清掉：新的一轮。
    pub fn clear(&mut self) {
        self.pending.clear();
        self.spans.clear();
    }

    /// 正在等的这一段从什么时候起；没在等的没有。
    pub fn waiting_since(&self) -> Option<Timestamp> {
        if self.pending.is_empty() {
            return None;
        }
        self.spans.last().map(|(start, _)| *start)
    }

    /// `from` 到 `to` 里等人的毫秒数：还在等的那一段算到 `to`。
    pub fn waited_ms(&self, from: Timestamp, to: Timestamp) -> u64 {
        self.spans
            .iter()
            .map(|(start, end)| {
                let start = start.unix_millis().max(from.unix_millis());
                let end = end.unwrap_or(to).unix_millis().min(to.unix_millis());
                u64::try_from(end - start).unwrap_or(0)
            })
            .sum()
    }

    /// `from` 到 `to` 刨掉等人的那几段，多少毫秒。
    pub fn busy_ms(&self, from: Timestamp, to: Timestamp) -> u64 {
        let all = u64::try_from(to.unix_millis() - from.unix_millis()).unwrap_or(0);
        all.saturating_sub(self.waited_ms(from, to))
    }
}

/// 一个会话里她干活的时间：每一轮从开始到结束，刨掉等人的那几段，加起来。撤销、压缩不减：干过就算。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Busy {
    /// 走完了的那几轮加起来的毫秒数。
    done: u64,
    /// 在跑的那一轮从什么时候开始。
    started: Option<Timestamp>,
    /// 在跑的那一轮里等人的几段。
    waits: Waits,
}

impl Busy {
    /// 看一条事件（提交时、载入时都喂，照日志的先后）。
    pub fn note(&mut self, event: &Event) {
        match &event.body {
            Body::TurnStarted(_) => {
                self.waits.clear();
                self.started = Some(event.at);
            }
            Body::TurnEnded(_) => {
                if let Some(started) = self.started.take() {
                    self.done = self
                        .done
                        .saturating_add(self.waits.busy_ms(started, event.at));
                }
                self.waits.clear();
            }
            _ => {
                self.waits.note(event);
            }
        }
    }

    /// 到 `now` 为止干活的毫秒数：在跑的那一轮算到 `now`。
    pub fn total_ms(&self, now: Timestamp) -> u64 {
        let running = self
            .started
            .map_or(0, |started| self.waits.busy_ms(started, now));
        self.done.saturating_add(running)
    }
}
