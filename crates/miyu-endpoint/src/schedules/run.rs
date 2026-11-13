//! 睡到点（施工 J-1 上）：一个后台任务，只睡到最早的那一次；表改了被叫醒、重算。到点的一个一个交进会话
//! （`fire.rs`），先把下一次写进表再交：她在那一轮里改下一次（`schedule_wakeup`，施工 J-2）不会被这里盖掉。人生循环（施工
//! LF-1）先写的是现在加默认间隔：她这一轮没定也照它再醒；交过以后只记上一次那几格（`record`），不带着旧的下一次写回去。

use std::sync::{Arc, Weak};

use miyu_kernel::time::Timestamp;
use miyu_store::schedules::{Loop, Schedule, Schedules as Table, Target};

use super::{TARGET, next, zone};
use crate::Core;

impl Core {
    /// 起唤醒表（施工 J-1 上）：开上已有的表、跳过错过的，然后在后台睡到点、到点交进会话。核心没了，它下一次醒来就收手。
    pub fn start_schedules(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        self.schedules.recall();
        tokio::spawn(run(Arc::downgrade(self)))
    }
}

/// 睡、醒、交，一直到核心没了。
async fn run(core: Weak<Core>) {
    loop {
        let (wake, clock) = {
            let Some(core) = core.upgrade() else {
                return;
            };
            (
                Arc::clone(&core.schedules.wake),
                Arc::clone(&core.schedules.clock),
            )
        };
        // 先挂上叫醒，再算最早的：算的时候表改了也叫得醒。
        let notified = wake.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let earliest = {
            let Some(core) = core.upgrade() else {
                return;
            };
            core.schedules.earliest()
        };
        match earliest {
            Some(at) => {
                tokio::select! {
                    () = &mut notified => continue,
                    () = clock.sleep_until(at) => {}
                }
            }
            None => {
                notified.await;
                continue;
            }
        }
        let Some(core) = core.upgrade() else {
            return;
        };
        let now = clock.now();
        if let Some(table) = core.schedules.opened() {
            let due = match table.due(now) {
                Ok(due) => due,
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "schedules not read");
                    Vec::new()
                }
            };
            let holds = &core.schedules.holds;
            for schedule in due.into_iter().filter(|schedule| {
                schedule
                    .next
                    .is_some_and(|next| holds.later(&schedule.id, next) <= now)
            }) {
                fire(&core, &table, schedule, now).await;
            }
        }
        drop(core);
        // 每一轮让一下：钟、表都不让等的时候（写不进、钟停在过去）也不霸着线程。
        tokio::task::yield_now().await;
    }
}

/// 到点的一个：先写下一次（跑完就没了的删掉），再交进会话，记上一次。
async fn fire(core: &Arc<Core>, table: &Table, schedule: Schedule, now: Timestamp) {
    let Some(due) = schedule.next else {
        return;
    };
    let once = next::once(&schedule.when) && !paced(schedule.looping);
    let mut ahead = schedule.clone();
    ahead.next = match schedule.looping {
        // 她自己定节奏的：交进去以后停着等她定（施工 J-2：她不调就不再醒）。
        Some(Loop::Dynamic) => None,
        // 人生：没定也照默认间隔再醒（施工 LF-1）；她在这一轮里定了的经 `set_next` 盖掉它。
        Some(Loop::Life) => next::plus(now, next::LIFE_GAP),
        Some(Loop::Fixed) | None => next::after(&schedule.when, due, now, &zone()),
    };
    // 错过的这一次推的时候一起报了（施工 J-4）：清零。
    if let Target::Package { missed } = &mut ahead.target {
        *missed = 0;
    }
    let written = if once {
        table.remove(&schedule.id).map(|_| ())
    } else {
        table.put(&ahead).map(|_| ())
    };
    if let Err(error) = written {
        // 写不进下一次的不交：交了又记不住，下一次醒来会再交一遍。先放一放再试，不然马上又醒、又写不进，空转（`hold.rs`）。
        core.schedules.holds.failed(&schedule.id, now);
        tracing::warn!(target: TARGET, schedule = %schedule.id, error = %error, "schedule not advanced");
        return;
    }
    core.schedules.holds.cleared(&schedule.id);
    let delivered = super::fire::deliver(core, &schedule, due).await;
    tracing::info!(
        target: TARGET,
        schedule = %schedule.id,
        result = %delivered.result,
        "schedule fired"
    );
    if !once {
        let last = miyu_store::schedules::Last {
            at: now,
            result: delivered.result,
            session: delivered.session,
        };
        if let Err(error) = table.record(&schedule.id, &last, delivered.stop) {
            tracing::warn!(target: TARGET, schedule = %schedule.id, error = %error, "schedule not recorded");
        }
    }
    core.schedules.changed();
    core.schedules.notify(&schedule.id);
}

/// 核心起来时已经过了的一个（错过的不补）：跑完就没了的删掉，她自己定节奏的停着等，人生的下一次写成现在、马上补醒一次（停了
/// 多久都只这一次，施工 LF-1），别的照现在算下一次；记一行。推给扩展的记下错过了几次，下一次推的时候报（施工 J-4）。
pub(super) fn skip(table: &Table, mut schedule: Schedule, now: Timestamp) {
    let due = schedule.next;
    tracing::info!(target: TARGET, schedule = %schedule.id, "schedule missed");
    if let (Target::Package { missed }, Some(due)) = (&mut schedule.target, due) {
        *missed = missed.saturating_add(next::missed(&schedule.when, due, now, &zone()));
    }
    let written = if next::once(&schedule.when) && !paced(schedule.looping) {
        table.remove(&schedule.id).map(|_| ())
    } else {
        schedule.next = match (schedule.looping, due) {
            (Some(Loop::Life), _) => Some(now),
            (Some(Loop::Dynamic), _) | (_, None) => None,
            (Some(Loop::Fixed) | None, Some(due)) => next::after(&schedule.when, due, now, &zone()),
        };
        table.put(&schedule).map(|_| ())
    };
    if let Err(error) = written {
        tracing::warn!(target: TARGET, schedule = %schedule.id, error = %error, "schedule not advanced");
    }
}

/// 下一次由她每轮定的循环（她自己定节奏的、人生）：「什么时候」是某个时刻也不是跑完就删。
fn paced(looping: Option<Loop>) -> bool {
    match looping {
        Some(Loop::Dynamic | Loop::Life) => true,
        Some(Loop::Fixed) | None => false,
    }
}
