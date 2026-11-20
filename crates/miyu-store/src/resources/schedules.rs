//! 到点交进来的话的标签（施工 J-1 上，`internal/blueprint/schedules.md` 第六节）：`software/schedule/common/`、`software/loop/common/`
//! 下各一对 `message-open.txt`、`message-close.txt`（施工 J-2 写）。造会话时冻结进快照的 `core.schedule`；缺一份的当没有，那种话
//! 照人的话原样渲染。
//! 光打 `/loop` 交进去的那句 `software/loop/autonomous.txt`（施工 LO-1）也不进快照：敲命令那一刻读，原样存进定时表那一行的 `prompt`。

use miyu_policy::ScheduleTexts;

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 四份都在才有。
    pub fn schedule_texts(&self) -> Option<ScheduleTexts> {
        let read =
            |package: &str, name: &str| self.read(&["software", package, "common", name]).ok();
        Some(ScheduleTexts {
            schedule_open: read("schedule", "message-open.txt")?,
            schedule_close: read("schedule", "message-close.txt")?,
            loop_open: read("loop", "message-open.txt")?,
            loop_close: read("loop", "message-close.txt")?,
        })
    }

    /// 光打 `/loop` 开的自主循环交进去的那句（施工 LO-1，`schedules.md` 第五节第 9 条）：没有人写的话，她照自己的判断接着做、
    /// 定下一次或者停下。原文，行尾的空白由用的一方去掉。
    ///
    /// # Errors
    ///
    /// 读不了这份文件。
    pub fn loop_autonomous(&self) -> Result<String, SourceError> {
        self.read(&["software", "loop", "autonomous.txt"])
    }
}

#[cfg(test)]
mod tests;
