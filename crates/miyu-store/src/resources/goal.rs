//! 长期目标续轮消息的几份（施工 X-7，X-8 上从 `resources.rs` 拆出来，那边到了行数上限），都在 `core/goal/` 下。

use super::{ResourceRoot, SourceError};

/// 续轮消息的几份（[`ResourceRoot::goal_rounds`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalRounds {
    /// 第一轮的完整版。
    pub full: String,
    /// 之后每轮的短版。
    pub again: String,
    /// 人用 `/goal` 设的：填进 `{set_by}`。
    pub by_person: String,
    /// 她自己用 `goal` 工具设的：填进 `{set_by}`。
    pub by_you: String,
}

impl ResourceRoot {
    /// 长期目标的续轮消息（施工 X-7，`26-提示词.md` 第十节）：第一轮的完整版 `core/goal/round.txt`、之后每轮的短版
    /// `core/goal/round-again.txt`，`{objective}`、`{round}`、`{set_by}` 由会话层填；`{set_by}` 照谁设的填
    /// `core/goal/set-by-person.txt` 或 `set-by-you.txt`（施工 X-8 上，末尾的换行去掉）。每开一轮读一次：改了文件下一轮就
    /// 照新的。
    ///
    /// # Errors
    ///
    /// 读不了其中一份，写明是哪一份。
    pub fn goal_rounds(&self) -> Result<GoalRounds, SourceError> {
        let read = |name: &str| self.read(&["core", "goal", name]);
        Ok(GoalRounds {
            full: read("round.txt")?,
            again: read("round-again.txt")?,
            by_person: read("set-by-person.txt")?.trim_end().to_string(),
            by_you: read("set-by-you.txt")?.trim_end().to_string(),
        })
    }
}
