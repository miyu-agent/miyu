//! 到点交进来的话的标签装进快照（施工 J-1 上，`docs/blueprint/schedules.md` 第六节）：`resources/software/schedule/common/`、
//! `resources/software/loop/common/` 下各两份原文（施工 J-2 写），`resources/software/life/common/` 下人生的一对（施工 LF-2）。
//! 以前造的快照里没有、这一版还没写字的读成没有：那种话照人的话原样渲染。人生那一对是后来加的两格，没有的不写，以前的快照
//! 读进来再写出去一个字节不变、指纹不变。

use std::collections::BTreeMap;

use miyu_assemble::ScheduleTexts as Rendered;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::BuildError;

/// 三对标签：定时任务和固定间隔的循环一对，她自己定节奏的循环一对，人生循环一对（施工 LF-2）。
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
    /// 人生循环的标签（施工 LF-2）：字段 `id`。和 [`ScheduleTexts::life_close`] 两格都有才用；施工 LF-2 以前造的没有，读成
    /// 没有、不写，字节不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life_open: Option<String>,
    /// 它的收尾（施工 LF-2）：同上。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub life_close: Option<String>,
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
            life: self
                .life_open
                .as_deref()
                .zip(self.life_close.as_deref())
                .map(|(life_open, life_close)| Ok((open(life_open)?, life_close.to_string())))
                .transpose()?,
        })
    }
}

#[cfg(test)]
mod tests;
