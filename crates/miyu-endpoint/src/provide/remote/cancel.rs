//! `tool.cancel` 什么时候发（施工 O-2 下；施工 SD-1 时从 `remote.rs` 挪出来，那边过了 500 行）：叫它停时发；回应没到就不等了的
//! （超时、这次调用的 future 被丢掉）丢掉它时发；回应先到的不发。

use serde_json::Value;

use crate::reverse::Peer;

/// `tool.cancel` 发到哪一步了。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Sending {
    /// 还没发，丢掉时发。
    Armed,
    /// 发过了。
    Sent,
    /// 回应到了，不用发。
    Done,
}

/// 发 `tool.cancel` 的：叫它停时发；回应没到就不等了的（超时、这次调用的 future 被丢掉）丢掉它时发。
pub(super) struct Cancel {
    pub(super) peer: Peer,
    pub(super) params: Value,
    pub(super) state: Sending,
}

impl Cancel {
    /// 还没发的发一次。
    pub(super) fn send(&mut self) {
        if self.state == Sending::Armed {
            self.state = Sending::Sent;
            if !self.peer.notify("tool.cancel", self.params.clone()) {
                tracing::warn!(target: "miyu::endpoint", "tool.cancel not sent");
            }
        }
    }
}

impl Drop for Cancel {
    fn drop(&mut self) {
        self.send();
    }
}
