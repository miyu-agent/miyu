//! 回复写到了输出上限（施工 3-10，`docs/blueprint/kernel/session.md`「回复怎么收」）。
//!
//! 驱动说照常说完、却是写到上限停的（[`crate::accumulate::Delta::Truncated`]）：回复里的工具调用一个都不派，回复记成
//! 被打断的（`call/settle.rs`）。只截在正文里的照被打断的回复接着写（`reply_cut`，会接着写的供应商从断处续写）；截掉了
//! 调用、什么正文都没写出来的，告诉她拆小了接着做（`output_limit`）。马上再请求，不等、不算步数。
//!
//! 一轮里连着 [`TRUNCATION_LIMIT`] 次以后，下一次再写到上限照出错收，分类 `output_limit`，结束这一轮：同样大的一步
//! 再来也是一样。照常说完、没写到上限的一次就清零。

use super::Session;
use super::action::Action;
use super::call::{Call, Truncation};
use super::turn::Stage;
use crate::event::{Body, CallError, ErrorClass, Event};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

/// 一轮里连着写到上限，最多接着几次（照 Claude Code 的恢复次数）。
pub(super) const TRUNCATION_LIMIT: u32 = 3;

impl Session {
    /// 这一次写到了上限、又已经连着接过 [`TRUNCATION_LIMIT`] 次：照出错收的那个错。别的没有。
    pub(super) fn output_limit(&self, call: &Call) -> Option<CallError> {
        let turn = self.turn.as_ref()?;
        (call.truncated() && turn.truncations >= TRUNCATION_LIMIT).then(|| CallError {
            class: ErrorClass::OutputLimit,
            message: format!("回复连着 {} 次写到了输出上限", TRUNCATION_LIMIT + 1),
            status: None,
        })
    }

    /// 写到上限的回复已经记下（`events`）：跟一条事实，回合回到「准备好」，落了盘照有效历史再请求。
    pub(super) fn truncated(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        mut events: Vec<Event>,
        truncation: Truncation,
    ) -> Vec<Action> {
        let fact = match truncation {
            Truncation::Text => Some(self.policy.facts.reply_cut()),
            Truncation::Work => self.policy.facts.output_limit(),
        };
        if let Some(fact) = fact {
            events.push(self.record(at, By::Kernel, cause, Body::ContextInjected(fact)));
        }
        if let Some(turn) = self.turn.as_mut() {
            turn.answered();
            turn.truncations += 1;
            turn.retrying = true;
            turn.stage = Stage::Ready;
        }
        events.extend(self.refresh_facts(at));
        vec![Action::Append(events)]
    }
}
