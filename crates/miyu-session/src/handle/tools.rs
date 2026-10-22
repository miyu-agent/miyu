//! 下一次请求会发的工具（施工 S-T，`session.tools`）：问 actor 要，它照回合开始那一套看一遍（`actor/persona/next.rs`）。

use tokio::sync::oneshot;

use miyu_policy::Offered;

use super::{Handle, Message, Stopped};

impl Handle {
    /// 下一次请求会发的工具，照发的先后。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn tools(&self) -> Result<Vec<Offered>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Tools(reply))?;
        answer.await.map_err(|_| Stopped)
    }
}
