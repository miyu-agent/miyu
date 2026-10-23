//! 投影里的几个小类型（施工 X-45 补从 `projector.rs` 挪出来：那一份到了行数上限）。

use miyu_kernel::event::{CallError, Usage};
use miyu_kernel::id::TurnId;
use miyu_kernel::time::Timestamp;

use crate::entry::EntryId;

/// 在跑的这一轮。
pub(super) struct Running {
    pub(super) id: TurnId,
    pub(super) started: Timestamp,
    pub(super) usage: Option<Usage>,
    pub(super) level: Option<String>,
    pub(super) failure: Option<CallError>,
    /// 没有触发的那一轮：手动压缩、清空，不另起收尾。
    pub(super) manual: bool,
    /// 这一轮她出过字、做过事。
    pub(super) spoke: bool,
}

/// 这次请求的一块。
#[derive(Debug, Clone)]
pub(super) enum Block {
    /// 正文：出了字才有条目。
    Text(Option<EntryId>),
    /// 思考：出了字才有条目。
    Thought(Option<EntryId>),
    /// 调工具：开始就有条目。
    Tool(EntryId),
}

/// 一件后台任务派出去时的样子。
#[derive(Debug, Clone)]
pub(super) struct Job {
    pub(super) what: miyu_kernel::event::JobKind,
    pub(super) title: String,
    pub(super) command: Option<String>,
}
