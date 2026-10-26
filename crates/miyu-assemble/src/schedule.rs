//! 到点的定时任务、循环交进来的话（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 2、4 条）：`by` 是 `schedule` 的
//! `message.user` 包一层带任务编号的标签，她分得清这句是到点了在叫她，不是人说的。定时任务、固定间隔的循环用一对，她自己定
//! 节奏的循环用另一对（收尾提醒她要接着就调 `schedule_wakeup`）。字在造快照时冻结（施工 J-2 写），以前造的快照没有的原样交回。

use std::collections::BTreeMap;

use miyu_kernel::block::Block;
use miyu_kernel::origin::{Schedule, ScheduleLoop};

use crate::tag::tagged;
use crate::texts::ScheduleTexts;

/// 任务 `schedule` 交进来的一句的块：包一层标签，标签里是任务编号；字以外的块接在后面。没有字的（以前造的快照、这一版还没
/// 写字）原样交回，和人的话一字不差。
pub(crate) fn message(
    schedule: &Schedule,
    blocks: Vec<Block>,
    texts: Option<&ScheduleTexts>,
) -> Vec<Block> {
    let Some(texts) = texts else {
        return blocks;
    };
    let (open, close) = match schedule.looping {
        Some(ScheduleLoop::Dynamic) => (&texts.loop_open, &texts.loop_close),
        Some(ScheduleLoop::Fixed) | None => (&texts.schedule_open, &texts.schedule_close),
    };
    let fields = BTreeMap::from([("id", schedule.schedule.as_str())]);
    // 造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
    let open = open.render(&fields).unwrap_or_default();
    tagged(open, blocks, close)
}

#[cfg(test)]
mod tests;
