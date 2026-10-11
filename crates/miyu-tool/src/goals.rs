//! 长期目标的端口（施工 X-8 上，`docs/blueprint/tools/goal.md`）：`goal` 经它看、设、结束这个会话的目标。实现在会话层：
//! 交给 actor，看的照会话状态答，改的交内核 `Goal` 命令（`by` 是这次调用），内核拒的原因原样交回来。

use std::fmt;
use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;

use miyu_kernel::session::Reason;

/// 工具名。
pub const GOAL: &str = "goal";

/// 续轮开的那一轮里报卡住，目标至少要开过几轮（`goal.md`「照旧版留下的」第 3 条）。
pub const BLOCKED_AFTER: u32 = 3;

/// 她要做的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalAsk {
    /// 看现在的目标。
    Get,
    /// 设一个：原文、轮数上限（不写的不限）。
    Set {
        /// 原文。
        text: String,
        /// 轮数上限。
        limit: Option<NonZeroU32>,
    },
    /// 报做完了。
    Complete,
    /// 报卡住了。
    Blocked {
        /// 卡在哪。
        reason: String,
    },
}

/// 目标现在的样子：给她看的那几样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalNow {
    /// 原文。
    pub text: String,
    /// 轮数上限；不限的没有。
    pub limit: Option<NonZeroU32>,
    /// 在哪一步，协议里的写法：`running`、`paused`、`completed`、`blocked`。
    pub state: &'static str,
    /// 卡在哪：卡住的才有。
    pub reason: Option<String>,
    /// 设下以后开过几轮。
    pub round: u32,
}

/// 端口答的：这时的目标（改的是改之前的那一个；没有的没有），这次调用在不在续轮开的那一轮里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalSeen {
    /// 这时的目标。
    pub now: Option<GoalNow>,
    /// 这次调用在续轮开的那一轮里。
    pub in_round: bool,
}

/// 没成的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalRefused {
    /// 续轮开的那一轮里报卡住，目标才开过 `round` 轮（不到 [`BLOCKED_AFTER`]）。
    TooEarly {
        /// 开过几轮。
        round: u32,
    },
    /// 内核拒了：已经有一个、没有目标、原文是空白的，别的照原因码。
    Kernel(Reason),
    /// 会话停了、这次调用了结了：没答到。
    Stopped,
}

/// 等端口答的 future。
pub type GoalAnswering<'a> =
    Pin<Box<dyn Future<Output = Result<GoalSeen, GoalRefused>> + Send + 'a>>;

/// 长期目标的端口：本机的主会话每次调用一个（`Agents::lists_sessions` 那几种会话）。
pub trait GoalPort: Send + Sync {
    /// 办 `ask`：看的交回这时的目标；改的落了盘才交回，交回改之前的目标。
    fn goal(&self, ask: GoalAsk) -> GoalAnswering<'_>;
}

impl fmt::Debug for dyn GoalPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GoalPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn GoalPort {
    fn eq(&self, other: &dyn GoalPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn GoalPort {}
