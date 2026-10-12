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
