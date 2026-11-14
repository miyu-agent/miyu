//! 人生会话的端口（施工 LF-4 下，设计 `70-推演人生.md` 第三节第 5 条，`docs/blueprint/life.md`「结束」）：她自己的 `end_life`
//! 经它结束这段人生。实现在协议端点（结束的那几步在端点的 `life::finish`），经 [`crate::SchedulePort::life`] 交给工具：人生的
//! 醒来本来就是定时表里人生那一行，交定时端口的地方都交了它，不另在造会话、载入、调用上下文里多拉一格。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::{CallId, SessionId};

/// 她自己结束这段人生的工具名。
pub const END_LIFE: &str = "end_life";

/// 端口没办成的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeRefused {
    /// 这个会话不是人生会话。
    NotLife,
    /// 这段人生已经结束了：不再记一次。
    Ended,
    /// 核心、会话这时用不了：没记下。
    Unavailable,
}

/// 等端口答的 future：成了的是这一次记下了结束。
pub type LifeEnding<'a> = Pin<Box<dyn Future<Output = Result<(), LifeRefused>> + Send + 'a>>;

/// 人生会话的端口：协议端点造一个，经定时端口交给工具。
pub trait LifePort: Send + Sync {
    /// 她在会话 `session` 上用调用 `call` 结束这段人生，遗言是 `last_words`（调的一方已经查过不是空的）。记下结束就答，不等后面
    /// 的清理：清理要等这一轮结束、会话封存，等了这一轮就结束不了。
    fn end(&self, session: &SessionId, call: &CallId, last_words: String) -> LifeEnding<'_>;
}

impl fmt::Debug for dyn LifePort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LifePort")
    }
}
