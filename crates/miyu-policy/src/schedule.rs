//! 到点交进来的话的标签装进快照（施工 J-1 上，`docs/blueprint/schedules.md`）：`resources/software/schedule/common/`、
//! `resources/software/loop/common/` 下各两份原文（施工 J-2 写）。以前造的快照里没有、这一版还没写字的读成没有：那种话照人的
//! 话原样渲染。

use std::collections::BTreeMap;

use miyu_assemble::ScheduleTexts as Rendered;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::BuildError;

/// 两对标签：定时任务和固定间隔的循环一对，她自己定节奏的循环一对。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleTexts {
    /// 定时任务的标签：字段 `id`，任务编号。
    pub schedule_open: String,
    /// 它的收尾。
    pub schedule_close: String,
    /// 循环的标签：字段 `id`。
    pub loop_open: String,
    /// 它的收尾。
    pub loop_close: String,
}

impl ScheduleTexts {
    /// 交给组装器的样子：两个标签读成模板，拿 `id` 试换一次。
    ///
    /// # Errors
    ///
    /// 标签的模板坏了，或者要了 `id` 以外的字段。
    pub(crate) fn rendered(&self) -> Result<Rendered, BuildError> {
        let bad = |error| BuildError::Texts {
            which: "schedule message texts",
            error,
        };
        let open = |text: &str| {
            let template = Template::parse(text).map_err(bad)?;
            template
                .render(&BTreeMap::from([("id", "")]))
                .map_err(bad)?;
            Ok::<_, BuildError>(template)
        };
        Ok(Rendered {
            schedule_open: open(&self.schedule_open)?,
            schedule_close: self.schedule_close.clone(),
            loop_open: open(&self.loop_open)?,
            loop_close: self.loop_close.clone(),
        })
    }
}
