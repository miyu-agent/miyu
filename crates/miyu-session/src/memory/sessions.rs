//! 整理会话的端口（施工 D-1 上，设计 `41-整理会话.md`）：合并任务经它开一趟整理会话、一批一轮地跑、叫停。会话表实现，核心起来时
//! 装进来（[`crate::Memory::give_dream_sessions`]）；没装的（测试里没接会话表的核心）照旧走一次性入口。

use miyu_kernel::id::{AccountId, CommandId, SessionId};

pub use miyu_kernel::event::DreamEnded as DreamResult;

use crate::route::Watched;
use crate::spawn::Pending;

/// 整理会话是哪一间的（设计 41 DS1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DreamRoom {
    /// 人格那一间：人格的编号。
    Persona(String),
    /// 会话那一间：哪个会话的记忆。
    Session(SessionId),
}

/// 开一趟整理会话要的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DreamSpec {
    /// 那一间的主人（设计 41 第二节第 8 条）：会话的属主。
    pub owner: AccountId,
    /// 哪一间。
    pub room: DreamRoom,
    /// 人叫的（`/dream`、`memory.dream`、`miyu memory dream`）；后台自己合的是假。
    pub manual: bool,
    /// 人在哪个会话里叫的（`/dream`）：那个会话的 `/stop` 照它叫停这一趟。
    pub from: Option<SessionId>,
    /// 照 `memory.organizer` 解析好的模型引用；没有的照这一刻的 `models.chat`。
    pub model: Option<String>,
}

/// 跑完的一批。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DreamBatch {
    /// 这一轮答完了：回答的原文（`message.assistant` 的文字块照先后接起来，思考不要）。
    Answered(String),
    /// 这一轮被打断了（叫停）：这一批放弃。
    Stopped,
    /// 这一轮出错结束了（`turn.ended` 的原因不是答完、不是打断）：英文的一句为什么。
    Failed(String),
}

/// 会话表那边替合并任务做的四样。
pub trait DreamSessions: Send + Sync + std::fmt::Debug {
    /// 开一趟：`session.created` 落了盘才交回编号。
    fn open(&self, spec: DreamSpec) -> Pending<'_, Result<SessionId, String>>;

    /// 跑一批：先清空（同 `/clear`），再以记忆模块的名义发 `text`（原样），等这一轮结束（`turn.ended` 落了盘）。`id` 是这一批
    /// 的命令编号，重发只算一次。正文边流边交给 `watch`（施工 D-1 下，同一次性入口的 `call_watching`：思考不交；换了一次请求
    /// 从头再来的先交 [`Watched::Restart`]），R-21 照它边合边推。核心在停、会话没了、日志写不进的交回 `Err`。
    fn batch<'a>(
        &'a self,
        session: SessionId,
        id: CommandId,
        text: String,
        watch: &'a mut (dyn FnMut(Watched<'_>) + Send),
    ) -> Pending<'a, Result<DreamBatch, String>>;

    /// 这一趟跑完了（施工 D-1 下，设计 41 DS9）：照删会话的路删掉这一间最旧的几趟（进回收处），只留最近 `keep` 趟（这一趟
    /// 也算；`0` 是连它一起删）。有回合在跑的不删。交回删了几个；没了的、删不成的记一行运行日志，不算错。`result` 是这一趟
    /// 做了什么（施工 DR-1）：有的先以记忆模块的名义在这一趟的日志末尾记一条 `dream.ended`，会话列表照它写 `result`；记不上的
    /// 记一行运行日志，照样删旧的。
    fn close(
        &self,
        session: SessionId,
        keep: usize,
        result: Option<DreamResult>,
    ) -> Pending<'_, Result<usize, String>>;

    /// 叫停：打断这个会话在跑的那一轮，[`DreamSessions::batch`] 那一头交回 [`DreamBatch::Stopped`]。
    fn stop(&self, session: SessionId, id: CommandId) -> Pending<'_, Result<(), String>>;
}
