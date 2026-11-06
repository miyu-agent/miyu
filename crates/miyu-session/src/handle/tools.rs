//! 下一次请求会发的工具（施工 S-T，`session.tools`）：问 actor 要，它照回合开始那一套看一遍（`actor/persona/next.rs`）。

use tokio::sync::oneshot;

use super::{Handle, Message, Stopped};

impl Handle {
    /// 下一次请求会发的工具，照发的先后。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn tools(&self) -> Result<Vec<String>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Tools(reply))?;
        answer.await.map_err(|_| Stopped)
    }
}

impl Handle {
    /// 发送前照这个会话自己判，号 `version` 的改动让不让它的前缀变（施工 PX-1，`actor/persona/check.rs`）。不变的，它看到的号已经
    /// 对到 `version`。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn check_prefix(&self, version: u64) -> Result<crate::prefix::PrefixCheck, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::PrefixCheck(version, reply))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 人换了人格、预设、已经先压缩过（施工 PX-1补）：放行到号 `version`，再叫会话照这一刻的文件记下下一个回合要用的快照，之后
    /// 发送前的干算照它比（[`Handle::check_prefix`]），别的人格的头像这类改动不再让它问。会话停了的不管：没有下一个回合了。
    pub fn settle_prefix(&self, version: u64) {
        self.prefix().allow(version);
        if self.send(Message::PrefixSettle).is_err() {
            tracing::debug!(target: crate::TARGET, "prefix not settled: the session stopped");
        }
    }
}
