//! 给头看的限额（施工 6-3 补，`docs/blueprint/kernel/session.md` 的 `context_limits()`、`protocol.md` 的 `subscribe`）：
//! 会话实际用的模型的窗口，和内核自己判到线用的那一条压缩线；推 `model.changed` 要的在跑的回合（施工 8-9）；这时的上下文
//! 用量（施工 8-15，`context_used()`）；还算数的最近一次压缩、清空替代到哪（施工 R-13 补，`compacted_upto()`）。只读，不出动作。

use serde::Serialize;

use super::{Action, Limits, Session};
use crate::id::{CommandId, Seq, TurnId};

mod breakdown;
pub use breakdown::{Breakdown, Frame, Said, breakdown_in};

/// 给头看的限额：头照它画「用量 / 窗口」、算离压缩还有多少。写成 JSON 就是协议里 `subscribe` 回应的 `limits`：
/// 没有的格不写，从不写 `null`（`protocol.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ContextLimits {
    /// 上下文窗口，token 数。没交过限额的、模型的资料没报的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<u64>,
    /// 压缩线（`compaction.md` 第二条第 2 条），和内核判到线用的是同一条。头不照公式自己算：公式里的输出预留、余量在
    /// 策略里。没有窗口的、策略里没有压缩的、算不出正数的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction_line: Option<u64>,
    /// 会话的上下文上限（施工 CTX-1，设计 68 第四节）：`window` 已经是套过它的生效的窗口，这一格让头写明是哪一格在起作用。
    /// 没设的没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
}

impl Session {
    /// 给头看的限额（施工 6-3 补）：执行器交完 `Input::Limits` 向内核要一份，交给头（`session/actor.md`）。
    pub fn context_limits(&self) -> ContextLimits {
        ContextLimits {
            window: self.limits.as_ref().and_then(|limits| limits.window),
            compaction_line: self.line(),
            context: self.context(),
        }
    }

    /// 会话的上下文上限至少要多少（施工 CTX-1，设计 68 第二节第 5 条）：照这时的策略和交过的限额里的最大输出，压缩线算得出的
    /// 最小窗口（[`super::Compaction::least_window`]）。策略里没有压缩的没有：不主动压，设多少都行。只读，不出动作。
    pub fn least_context(&self) -> Option<u64> {
        let max_output = self.limits.as_ref().and_then(|limits| limits.max_output);
        self.policy.compaction.as_ref()?.least_window(max_output)
    }

    /// 这时的上下文用量（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 7 条）：照有效历史组装这时的请求，用和压缩线
    /// 同一个算法估（`compaction.md` 第一条：锚加上锚以后的本地估算）。她自己查用量的工具（`session_usage`）派出去时，执行器
    /// 向内核要一份。策略里没有压缩、没交过限额的算不了，没有。只读，不出动作。
    pub fn context_used(&self) -> Option<u64> {
        self.used(&self.assembled())
    }

    /// 照有效历史组装这时的请求（施工 YC-14）：和发主请求时组装的是同一步，还没放转述、没写输出上限。执行器开着 `DEBUG`
    /// 时重跑一遍，量它用多久（内核自己不读时钟，`session/actor.md`「运行日志」的 `request timed`）。只读，不出动作。
    pub fn assembled(&self) -> crate::request::Request {
        self.policy.assembler.assemble(&self.history)
    }

    /// 在跑的回合和它的 `cause`（施工 8-9）：会话 actor 推 `model.changed` 时照它写（`by` 是内核，`cause` 是回合的，
    /// `models.md`「瞬时事件」）。没有在跑的回合的没有。只读，不出动作。
    pub fn turn_cause(&self) -> Option<(TurnId, Option<CommandId>)> {
        self.turn.as_ref().map(|turn| (turn.id, turn.cause.clone()))
    }

    /// 还算数的最近一次压缩、清空替代到哪（施工 R-13 补）：这以后开的轮还原样在上下文里，以前的她只看得见摘要（清空的连摘要
    /// 都没有）。没压缩、清空过的，或者都撤掉了的，没有。她用 `memory_search` 搜以前的对话只跳过这个会话这以后的几轮：执行器
    /// 派这一次调用时向内核要一份（`memory.md`「工具」）。只读，不出动作。
    pub fn compacted_upto(&self) -> Option<Seq> {
        self.ledger.compacted()
    }
}

impl Session {
    /// 执行器交来的限额：换上。这一轮里发给谁变了的（施工 E-M：换了模型、档位、预设），下一次请求之前把事实查一遍，模型那一块
    /// 跟着追加。
    pub(super) fn take_limits(&mut self, limits: Limits) -> Vec<Action> {
        let moved = self.limits.as_ref().map(|old| &old.identity) != Some(&limits.identity);
        self.limits = Some(limits);
        if let Some(turn) = self.turn.as_mut().filter(|_| moved) {
            turn.refresh = true;
        }
        Vec::new()
    }

    /// 到 `now` 为止她在这个会话里干活的毫秒数（施工 E-M 下）：每一轮从开始到结束，等人批准、等人回答的那几段不算；在跑的那一轮
    /// 算到 `now`。`session_usage` 派出去时执行器向内核要一份。只读，不出动作。
    pub fn busy_ms(&self, now: crate::time::Timestamp) -> u64 {
        self.busy.total_ms(now)
    }

    /// 这一轮发给谁（施工 E-M）：执行器交过的限额里的；没交过的没有。
    pub(super) fn identity(&self) -> Option<&crate::facts::Identity> {
        self.limits.as_ref()?.identity.as_ref()
    }
}
