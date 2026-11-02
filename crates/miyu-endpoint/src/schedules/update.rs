//! 改一项（施工 J-5，`docs/blueprint/schedules.md` 第四节、第五节第 3a 条）：换时间、换那句话，编号、建的时刻、上一次、跑在哪
//! 都不变。以前只能删了重建：编号换了、上一次没了，她那边循环的 `by.loop` 也对不上。

use miyu_kernel::time::Timestamp;

use super::{Loop, Schedule, ScheduleError, Schedules, When, checked, next, zone};

impl Schedules {
    /// 改编号是 `id` 的那一个：`when` 写了的照建时一样查、换掉，没停的照现在重算下一次（错过的不补），停着的还停着；`prompt`
    /// 写了的换掉那句话。都不写的照旧写回去。交回改好的一行。
    ///
    /// # Errors
    ///
    /// 没有这个编号；cron 读不成、时区不认识、隔得太短、算不出下一次；循环改了种类（[`ScheduleError::LoopKind`]）；表用不了、
    /// 写不了。出错的什么都不改。
    pub fn update(
        &self,
        id: &str,
        when: Option<When>,
        prompt: Option<String>,
    ) -> Result<Schedule, ScheduleError> {
        let when = when
            .map(|mut when| checked(&mut when).map(|()| when))
            .transpose()?;
        let updated = self.edit(id, |schedule, now| {
            if let Some(when) = when {
                renew(schedule, when, now)?;
            }
            if let Some(prompt) = prompt {
                schedule.prompt = prompt;
            }
            Ok(())
        })?;
        tracing::info!(target: super::TARGET, schedule = %id, "schedule updated");
        Ok(updated)
    }
}

/// 换掉 `schedule` 的时间：循环不改种类，没停的照 `now` 重算下一次。
fn renew(schedule: &mut Schedule, when: When, now: Timestamp) -> Result<(), ScheduleError> {
    if !keeps_kind(schedule.looping, &when) {
        return Err(ScheduleError::LoopKind);
    }
    if !schedule.paused {
        schedule.next = Some(next::first(&when, now, &zone()).ok_or(ScheduleError::NoNext)?);
    }
    schedule.when = when;
    Ok(())
}

/// 循环的种类跟着「什么时候」的写法（`/loop` 建的，`commands/looping.rs`）：她自己定节奏的是某个时刻（下一次由她每轮定），固定
/// 间隔的是每隔一阵。换成别的写法就成了另一种循环，她那边对不上，不收。定时任务（不是循环）哪种都行。
fn keeps_kind(looping: Option<Loop>, when: &When) -> bool {
    match looping {
        None => true,
        Some(Loop::Dynamic) => matches!(when, When::At { .. }),
        Some(Loop::Fixed) => matches!(when, When::Every { .. }),
    }
}
