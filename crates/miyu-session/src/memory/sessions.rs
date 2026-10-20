//! 整理会话的端口（施工 D-1 上，设计 `41-整理会话.md`）：合并任务经它开一趟整理会话、一批一轮地跑、叫停。会话表实现，核心起来时
//! 装进来（[`crate::Memory::give_dream_sessions`]）；没装的（测试里没接会话表的核心）照旧走一次性入口。

use miyu_kernel::id::{AccountId, CommandId, SessionId};

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

/// 会话表那边替合并任务做的三样。
pub trait DreamSessions: Send + Sync + std::fmt::Debug {
    /// 开一趟：`session.created` 落了盘才交回编号。顺手照 `memory.dream_keep` 删掉这一间最旧的几趟（正在跑的不删）。
    fn open(&self, spec: DreamSpec) -> Pending<'_, Result<SessionId, String>>;

    /// 跑一批：先清空（同 `/clear`），再以记忆模块的名义发 `text`（原样），等这一轮结束（`turn.ended` 落了盘）。`id` 是这一批
    /// 的命令编号，重发只算一次。核心在停、会话没了、日志写不进的交回 `Err`。
    fn batch(
        &self,
        session: SessionId,
        id: CommandId,
        text: String,
    ) -> Pending<'_, Result<DreamBatch, String>>;

    /// 叫停：打断这个会话在跑的那一轮，[`DreamSessions::batch`] 那一头交回 [`DreamBatch::Stopped`]。
    fn stop(&self, session: SessionId, id: CommandId) -> Pending<'_, Result<(), String>>;
}
