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
}
