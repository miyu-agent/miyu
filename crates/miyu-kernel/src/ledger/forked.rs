//! fork 子代理的账本（施工 A-F 上，`session/fork/within.rs`）：分叉点在父会话这一轮当中，子会话接着那一轮收尾。

use super::Ledger;
use crate::id::Seq;

impl Ledger {
    /// 分叉点在一轮当中的（fork 子代理）：开着的那一轮、那一步等结果的调用、最近一条回复照父会话到分叉点为止的账本 `parent`，
    /// 子会话给它们记占位结果、收住那一轮。那一轮不进能撤的几轮：它是父会话开的，子会话撤不到它。
    #[must_use]
    pub fn continuing(mut self, parent: &Ledger) -> Ledger {
        self.open = parent.open;
        self.pending = parent.pending.clone();
        self.last_reply = parent.last_reply;
        self
    }

    /// 最近一条回复（`message.assistant`）是第几条：正在进行的那一轮里还等结果的调用都是它派的（一步的调用都有了结果才请求
    /// 下一步）。fork 子代理照它定分叉点。
    pub fn last_reply(&self) -> Option<Seq> {
        self.last_reply
    }
}
