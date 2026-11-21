//! 会话 actor、会话表问的两样纯查询（从 `session.rs` 挪来，施工 LW-10 下：那个文件到了行数上限）：落了盘的最后一条、删得了没有。

use super::{Reason, Session};
use crate::id::Seq;

impl Session {
    /// 落了盘的最后一条（施工 3-8 六补）：`Stored` 送进来那一刻就推送了，所以也是推过的最后一条；还没落过盘的没有，载入的
    /// 是日志里最后一条。纯查询：会话 actor 订阅时照它定补发补到哪一条（`internal/blueprint/session/actor.md` 第 6 条）。
    pub fn landed(&self) -> Option<Seq> {
        self.stored
    }

    /// 删得了没有（施工 3-8 三补，`protocol.md` 的 `session.delete`）：不空闲的删不了，正在读回日志的是
    /// [`Reason::Restoring`]，别的（有回合在进行、结束了 `turn.ended` 还没落盘）是 [`Reason::TurnRunning`]。纯查询：会话
    /// actor 照它答应删、停下，挪目录是会话表的事。
    ///
    /// # Errors
    ///
    /// 删不了，交回原因。
    pub fn deletable(&self) -> Result<(), Reason> {
        if self.reading.is_some() {
            return Err(Reason::Restoring);
        }
        match self.idle() {
            true => Ok(()),
            false => Err(Reason::TurnRunning),
        }
    }
}
