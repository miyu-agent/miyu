//! 技能那一块的两份（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 2 条），都在 `core/skills/` 下。

use super::{ResourceRoot, SourceError};

/// 技能那一块的字（[`ResourceRoot::skill_texts`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillTexts {
    /// 头一行（`open.txt`）。
    pub open: String,
    /// 一个技能一行（`line.txt`）：字段 `name`、`description`、`path`。
    pub line: String,
    /// 会话中途列表变了、尾巴上那一份的外壳头（`changed-open.txt`，施工 X-11 下）。
    pub changed_open: String,
    /// 同上，外壳尾（`changed-close.txt`）。
    pub changed_close: String,
    /// 一个都没有了（`none.txt`）：外壳里只有这一句。
    pub none: String,
}

impl ResourceRoot {
    /// 她的 system 里技能那一块的字（施工 X-10 上，`26-提示词.md` 第十节）：造会话时读，拼好的钉进快照；会话中途列表变了时
    /// 尾巴上那一份的外壳（施工 X-11 下）回合开始读。
    ///
    /// # Errors
    ///
    /// 读不了其中一份，写明是哪一份。
    pub fn skill_texts(&self) -> Result<SkillTexts, SourceError> {
        let read = |name: &str| self.read(&["core", "skills", name]);
        Ok(SkillTexts {
            open: read("open.txt")?,
            line: read("line.txt")?,
            changed_open: read("changed-open.txt")?,
            changed_close: read("changed-close.txt")?,
            none: read("none.txt")?,
        })
    }
}

/// 人点名用一个技能（`/名字`，施工 PT-2 下）时套在正文外面的字（[`ResourceRoot::invoked_texts`]），都在 `core/skills/` 下。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvokedTexts {
    /// 外壳头（`invoked-open.txt`）：字段 `name`、`path`。
    pub open: String,
    /// 外壳尾（`invoked-close.txt`）。
    pub close: String,
    /// 人跟在名字后面写的（`invoked-arguments.txt`）：字段 `arguments`；没写的不带这一行。
    pub arguments: String,
}

impl ResourceRoot {
    /// 人点名用技能时的外壳（施工 PT-2 下，`26-提示词.md` 第十节）：点名的那一次读，不常驻。
    ///
    /// # Errors
    ///
    /// 读不了其中一份，写明是哪一份。
    pub fn invoked_texts(&self) -> Result<InvokedTexts, SourceError> {
        let read = |name: &str| self.read(&["core", "skills", name]);
        Ok(InvokedTexts {
            open: read("invoked-open.txt")?,
            close: read("invoked-close.txt")?,
            arguments: read("invoked-arguments.txt")?,
        })
    }
}
