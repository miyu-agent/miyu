//! 这时的上下文一块块的（施工 CX-1 上，`session.context`）：问 actor 要，它向内核要一份明细（`breakdown.rs`），再照现在的
//! 快照把 system 里认得出的几块、要先加载的工具标出来（`actor/persona/context.rs`）。只看，不换快照。

use tokio::sync::oneshot;

use miyu_kernel::session::Breakdown;
use miyu_policy::Offered;

use super::{Handle, Message, Stopped};

/// system 里的一块（施工 CX-1 下起照拼的先后全拆；照现在的料重拼和快照对不上的，只拆快照里单存了原文的几块，剩下的都是其他）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemPart {
    /// 人设。
    Persona,
    /// 子会话的场所说明。
    Venue,
    /// 群会话的格式说明。
    Group,
    /// 核心的几行：权限、本机文件的路径、本机是什么系统、跑的是哪一版。
    Core,
    /// 什么时候写成调工具的程序那一段。
    Clicall,
    /// show 的那一句（施工 D-5 补二）。
    Show,
    /// 风格锁。
    StyleLock,
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

/// 头问的是哪一样（施工 CX-3：两样走同一条消息）。
#[derive(Debug)]
pub(crate) enum Asked {
    /// 这时的上下文一块块的。
    Now(oneshot::Sender<Context>),
    /// 累计用量照上下文的块摊（`session.usage_parts`）。
    Spent(oneshot::Sender<Spent>),
}

/// 一次主请求（施工 CX-3）：发出去那一刻的上下文一块块的，和供应商报的输入（没命中、命中、写缓存加起来）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charged {
    /// 那一刻的明细：限额、锚、用量不算。
    pub context: Context,
    /// 报的输入。
    pub input: u64,
}

/// 这个会话的累计用量照上下文的块摊（施工 CX-3，`session.usage_parts`）：只算主请求（同 `usage.query` 的 `use` 是
/// `main`、子会话的是 `subagent`），照日志一条条回放。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spent {
    /// 主请求有几次（报没报用量都算，同 `usage.query` 的 `requests`）。
    pub requests: u64,
    /// 报了用量的那几次，每次一份。
    pub charged: Vec<Charged>,
    /// 输出加起来。
    pub output: u64,
}

impl Handle {
    /// 累计用量照上下文的块摊（施工 CX-3）。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn usage_parts(&self) -> Result<Spent, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Context(Asked::Spent(reply)))?;
        answer.await.map_err(|_| Stopped)
    }

    /// 这时的上下文一块块的。
    ///
    /// # Errors
    ///
    /// 会话停了。
    pub async fn context(&self) -> Result<Context, Stopped> {
        let (reply, answer) = oneshot::channel();
        self.send(Message::Context(Asked::Now(reply)))?;
        answer.await.map_err(|_| Stopped)
    }
}
