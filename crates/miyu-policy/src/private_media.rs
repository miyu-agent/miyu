//! 私聊里带的东西的记号要用的字（施工 O-35，`internal/blueprint/kernel/request.md`「群里的一行」私聊那一条）：语音那一句
//! （`core/venues/voice.txt`，O-33 冻进群会话的同一份），造会话时冻结在策略快照的 `core.private_media`。放在 `core` 这一层：
//! 私聊会话的快照没有 `group`（同 `core.forward`，施工 O-34）。它也是「O-35 起造的会话」的标志：有它的私聊，带了东西的那条
//! 写出 `[msg=…]` 和记号那一行；以前造的快照里没有，读成没有，私聊照以前渲染，一个字节不变。

use miyu_assemble::PrivateMedia as Rendered;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

/// 私聊里带的东西的记号要用的字（`core.private_media`），原文，行尾的换行也算。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateMediaTexts {
    /// 语音后面接的那一句（`core/venues/voice.txt` 的原文）。
    pub voice: String,
    /// 戳一戳那一句（施工 VP-1，`core/venues/poke.txt` 的原文，同群会话的 `poke`）。VP-1 以前造的没有，戳一戳只写记号；没有的
    /// 不写，字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poke: Option<String>,
}

impl PrivateMediaTexts {
    /// 组装器要的：语音那一句去掉行尾的空白（同群会话的 `voice`），戳一戳那一句读成模板（`group::poke`）。
    ///
    /// # Errors
    ///
    /// 戳一戳那一句写错了、字段不对：快照坏了。
    pub fn texts(&self) -> Result<Rendered, BuildError> {
        Ok(Rendered {
            voice: self.voice.trim_end().to_string(),
            poke: self.poke.as_deref().map(crate::group::poke).transpose()?,
        })
    }
}

impl Snapshot {
    /// 交给组装器的私聊记号的字：快照里有的才有。
    pub(crate) fn private_media(&self) -> Result<Option<Rendered>, BuildError> {
        self.core
            .private_media
            .as_ref()
            .map(PrivateMediaTexts::texts)
            .transpose()
    }
}

#[cfg(test)]
mod tests;
