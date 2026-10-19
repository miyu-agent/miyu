//! 人叫的整理，把手这一头（施工 R-7 补；R-22 加叫停，从 `handle.rs` 挪出来）：现在就整理这个会话的记忆、叫停这个会话叫的
//! 那几趟（`docs/blueprint/memory.md` 第七条第 9、13 款），actor 那一头在 `actor/extract.rs`。

use tokio::sync::oneshot;

use super::{Handle, Message, Stopped};
use crate::memory::{Dreamed, NotDreamed};

/// 人叫的整理交给 actor 的一件。
#[derive(Debug)]
pub(crate) enum Dream {
    /// 现在就整理：先抽这个会话还没抽的，再合，交回几样数。
    Now(oneshot::Sender<Result<Dreamed, NotDreamed>>),
    /// 叫停这个会话这时正在跑的整理，交回有没有在跑的。
    Stop(oneshot::Sender<bool>),
}

impl Handle {
    /// 现在就整理这个会话的记忆（施工 R-7 补，`/dream`、带 `session` 的 `memory.dream`，`memory.md` 第七条第 9 款）：先把这个
    /// 会话还没抽的那一段抽了（不等闲、答了一轮就算；正忙的、正在抽的不抽），再不看会话数、间隔现在就合，交回几样数。叫停了的
    /// 交回叫停以前合完了的，带 `stopped`（施工 R-22）。
    ///
    /// # Errors
    ///
    /// 会话停了。里面那一层：这个会话的记忆关着、人格记忆没装（`Off`），这一间正在合（`Busy`），没合成（`Failed`）。
    pub async fn dream(&self) -> Result<Result<Dreamed, NotDreamed>, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Dream(Dream::Now(reply)))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 叫停这个会话这时正在跑的整理（施工 R-22，`/dream stop`，`memory.md` 第七条第 13 款）：合完了的批留着，正在合的那一批
    /// 放弃、下次再合。交回有没有在跑的；没有的什么都不做。`/stop`（[`Handle::stop_jobs`]）也叫停它。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn stop_dream(&self) -> Result<bool, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Dream(Dream::Stop(reply)))?;
        answer.await.map_err(|_| Stopped)
    }
}
