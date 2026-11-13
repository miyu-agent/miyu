//! 到点交进来的话的标签（施工 J-1 上，`docs/blueprint/schedules.md` 第六节）：`software/schedule/common/`、`software/loop/common/`
//! 下各一对 `message-open.txt`、`message-close.txt`（施工 J-2 写），`software/life/common/` 下人生的一对（施工 LF-2）。造会话时
//! 冻结进快照的 `core.schedule`；前四份缺一份的当没有，那种话照人的话原样渲染；人生那一对缺一份的只当人生的没有。出厂的推演
//! 方法 `software/life/method.txt`（施工 LF-2）不进快照：到点那一刻读（`schedules.md` 第三节第 4 条）。

use miyu_policy::ScheduleTexts;

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 前四份都在才有；人生那一对两份都在才有。
    pub fn schedule_texts(&self) -> Option<ScheduleTexts> {
        let read =
            |package: &str, name: &str| self.read(&["software", package, "common", name]).ok();
        let life = read("life", "message-open.txt").zip(read("life", "message-close.txt"));
        let (life_open, life_close) = life.unzip();
        Some(ScheduleTexts {
            schedule_open: read("schedule", "message-open.txt")?,
            schedule_close: read("schedule", "message-close.txt")?,
            loop_open: read("loop", "message-open.txt")?,
            loop_close: read("loop", "message-close.txt")?,
            life_open,
            life_close,
        })
    }

    /// 出厂的推演方法（施工 LF-2，设计 `70-推演人生.md` 第三节第 3 条）：人格没有自己的 `prompts/life.md` 时，人生循环醒来那段话
    /// 的正文照它。原文，行尾的空白由拼正文的一方去掉。
    ///
    /// # Errors
    ///
    /// 读不了这份文件。
    pub fn life_method(&self) -> Result<String, SourceError> {
        self.read(&["software", "life", "method.txt"])
    }
}

#[cfg(test)]
mod tests;
