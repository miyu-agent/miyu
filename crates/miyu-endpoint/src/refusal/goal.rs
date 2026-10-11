//! 长期目标的拒绝（施工 X-8 下，`goal.md`「在新的架构里怎么放」第 8 条）。

use super::{REFUSED, Refusal};

impl Refusal {
    /// 这个会话没有长期目标（施工 X-8 下）：长期目标没装、预设没开，群里、私聊、子代理的会话。`/goal` 回它，`data.why` 说具体。
    pub(crate) const GOAL_UNAVAILABLE: Refusal = Refusal {
        code: REFUSED,
        reason: "goal_unavailable",
        data: None,
    };
}
