//! 整理会话跑完的那一条（施工 DR-1，设计 `41-整理会话.md`，记忆会话 2026-10-21 定的形状）：合并任务跑完一趟，以记忆模块的名义记
//! 在这一趟整理会话的日志末尾：给了几条、改了几条、作废了几条、有没有写摘要、是不是叫停的。会话列表照它给这一趟写 `result`；
//! 没跑完、崩了没记上的没有这一条。不进她的上下文。

use serde::{Deserialize, Serialize};

/// `dream.ended`：这一趟整理做了什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DreamEnded {
    /// 新记了几条。
    pub given: u32,
    /// 改了几条。
    pub revised: u32,
    /// 作废了几条。
    pub retired: u32,
    /// 写了这一间的摘要。
    pub summary: bool,
    /// 半路叫停的：做了的照数，没做完的不算。不是的不写。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stopped: bool,
}
