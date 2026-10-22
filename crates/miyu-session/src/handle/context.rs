//! 这时的上下文一块块的（施工 CX-1 上，`session.context`）：问 actor 要，它向内核要一份明细（`breakdown.rs`），再照现在的
//! 快照把 system 里认得出的几块、要先加载的工具标出来（`actor/persona/context.rs`）。只看，不换快照。

use tokio::sync::oneshot;

use miyu_kernel::session::Breakdown;
use miyu_policy::Offered;

use super::{Handle, Message, Stopped};

/// system 里的一块：快照里单存了原文的几块，剩下的都是其他。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemPart {
    /// 技能那一块。
    Skills,
    /// 知识库那一块。
    Knowledge,
    /// 「关于你」。
    Profile,
    /// 项目的附加提示词。
    Project,
    /// 扩展放在尾部的一段：包的编号。
    Appended(String),
    /// 剩下的：人设、场所说明、内核的几句……
    Other,
}

/// 这时的上下文一块块的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// 内核算的明细。
    pub breakdown: Breakdown,
    /// system 拆开：认得出的几块照快照里的先后，最后是其他；加起来就是 `breakdown.system`。一块都认不出的只有其他。
    pub system: Vec<(SystemPart, u64)>,
    /// 现在的工具面，照发的先后：`loaded` 是假的只露名字，要先加载。
    pub offered: Vec<Offered>,
}

impl Handle {
    /// 这时的上下文一块块的。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn context(&self) -> Result<Context, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Context(reply))?;
        answer.await.map_err(|_| Stopped)
    }
}
