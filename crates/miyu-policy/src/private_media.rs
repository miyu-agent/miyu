//! 私聊里带的东西的记号要用的字（施工 O-35，`docs/blueprint/kernel/request.md`「群里的一行」私聊那一条）：语音那一句
//! （`core/venues/voice.txt`，O-33 冻进群会话的同一份），造会话时冻结在策略快照的 `core.private_media`。放在 `core` 这一层：
//! 私聊会话的快照没有 `group`（同 `core.forward`，施工 O-34）。它也是「O-35 起造的会话」的标志：有它的私聊，带了东西的那条
//! 写出 `[msg=…]` 和记号那一行；以前造的快照里没有，读成没有，私聊照以前渲染，一个字节不变。

use miyu_assemble::PrivateMedia as Rendered;
use serde::{Deserialize, Serialize};

use crate::snapshot::Snapshot;

/// 私聊里带的东西的记号要用的字（`core.private_media`），原文，行尾的换行也算。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateMediaTexts {
    /// 语音后面接的那一句（`core/venues/voice.txt` 的原文）。
    pub voice: String,
}

impl PrivateMediaTexts {
    /// 组装器要的：语音那一句去掉行尾的空白（同群会话的 `voice`）。
    pub fn texts(&self) -> Rendered {
        Rendered {
            voice: self.voice.trim_end().to_string(),
        }
    }
}

impl Snapshot {
    /// 交给组装器的私聊记号的字：快照里有的才有。
    pub(crate) fn private_media(&self) -> Option<Rendered> {
        self.core
            .private_media
            .as_ref()
            .map(PrivateMediaTexts::texts)
    }
}

#[cfg(test)]
mod tests;
