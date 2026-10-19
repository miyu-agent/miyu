//! 场所会话用的几份（施工 O-13 中、下，O-33，O-34；O-34 从 `resources.rs` 拆出来，那边超了行数）：群会话的格式说明、群会话
//! 钉下的字，合并转发那几行的三句，私聊记号的语音那一句（施工 O-35），都在 `core/venues/` 下。

use miyu_policy::{ForwardTexts, GroupChat, GroupRecent, PrivateMediaTexts};

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 群会话的格式说明（施工 O-13 中）：`core/venues/group.txt` 的原文，造群会话时接在人设后面（`Snapshot::with_group`）。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn group_note(&self) -> Result<String, SourceError> {
        self.read(&["core", "venues", "group.txt"])
    }

    /// 群会话钉下的（施工 O-13 中）：时区 `offset`（比 UTC 早多少分钟），空的一条写什么照 `core/venues/no-text.txt` 的原文；
    /// 群聊近况（施工 O-13 下）的块头、缺口提示照 `core/venues/recent-open.txt`、`recent-omitted.txt` 的原文，预算是出厂的；
    /// 语音那一句（施工 O-33）照 `core/venues/voice.txt` 的原文。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn group_chat(&self, offset: i32) -> Result<GroupChat, SourceError> {
        Ok(GroupChat {
            offset,
            no_text: self.read(&["core", "venues", "no-text.txt"])?,
            recent: Some(GroupRecent {
                open: self.read(&["core", "venues", "recent-open.txt"])?,
                omitted: self.read(&["core", "venues", "recent-omitted.txt"])?,
                budget: miyu_policy::RECENT_BUDGET,
                limit: None,
            }),
            voice: Some(self.read(&["core", "venues", "voice.txt"])?),
        })
    }

    /// 合并转发那几行的三句（施工 O-34）：`core/venues/forward/` 下 `label.txt`、`numbered.txt`、`more.txt` 的原文。造会话时
    /// 冻结进快照的 `core.forward`；判官看的记录（`venue.records`）照快照里没有群会话的字的，也照它这时的。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn forward(&self) -> Result<ForwardTexts, SourceError> {
        Ok(ForwardTexts {
            label: self.read(&["core", "venues", "forward", "label.txt"])?,
            numbered: self.read(&["core", "venues", "forward", "numbered.txt"])?,
            more: self.read(&["core", "venues", "forward", "more.txt"])?,
        })
    }

    /// 私聊里带的东西的记号要用的（施工 O-35）：语音那一句照 `core/venues/voice.txt` 的原文（同群会话钉下的那一份）。造会话时
    /// 冻结进快照的 `core.private_media`。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn private_media(&self) -> Result<PrivateMediaTexts, SourceError> {
        Ok(PrivateMediaTexts {
            voice: self.read(&["core", "venues", "voice.txt"])?,
        })
    }
}
