//! 到点交进来的话的标签（施工 J-1 上，`docs/blueprint/schedules.md`）：`software/schedule/common/`、`software/loop/common/` 下
//! 各一对 `message-open.txt`、`message-close.txt`（施工 J-2 写）。造会话时冻结进快照的 `core.schedule`；四份缺一份的当没有，
//! 那种话照人的话原样渲染。

use miyu_policy::ScheduleTexts;

use super::ResourceRoot;

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
}
