//! 睡到点（施工 J-1 上）：一个后台任务，只睡到各账号里最早的那一次；表改了被叫醒、重算。到点的一个一个交进会话
//! （`fire.rs`），先把下一次写进表再交：她在那一轮里改下一次（`schedule_wakeup`，施工 J-2）不会被这里盖掉。

use std::sync::{Arc, Weak};

use miyu_kernel::id::AccountId;
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
        for (account, table) in core.schedules.opened() {
            let due = match table.due(now) {
                Ok(due) => due,
                Err(error) => {
                    tracing::warn!(target: TARGET, account = %account, error = %error, "schedules not read");
                    continue;
                }
            };
            let holds = &core.schedules.holds;
            for schedule in due.into_iter().filter(|schedule| {
                schedule
                    .next
                    .is_some_and(|next| holds.later(&account, &schedule.id, next) <= now)
            }) {
                fire(&core, &account, &table, schedule, now).await;
            }
        }
        drop(core);
        // 每一轮让一下：钟、表都不让等的时候（写不进、钟停在过去）也不霸着线程。
        tokio::task::yield_now().await;
    }
}

/// 到点的一个：先写下一次（跑完就没了的删掉），再交进会话，记上一次。
async fn fire(
    core: &Arc<Core>,
    account: &AccountId,
    table: &Table,
    schedule: Schedule,
    now: Timestamp,
) {
    let Some(due) = schedule.next else {
        return;
    };
    let once = next::once(&schedule.when) && schedule.looping != Some(Loop::Dynamic);
    let mut ahead = schedule.clone();
    ahead.next = match schedule.looping {
        // 她自己定节奏的：交进去以后停着等她定（施工 J-2：她不调就不再醒）。
        Some(Loop::Dynamic) => None,
        _ => next::after(&schedule.when, due, now, &zone()),
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
        core.schedules.holds.failed(account, &schedule.id, now);
        tracing::warn!(target: TARGET, account = %account, schedule = %schedule.id, error = %error, "schedule not advanced");
        return;
    }
    core.schedules.holds.cleared(account, &schedule.id);
    let delivered = super::fire::deliver(core, account, &schedule, due).await;
    tracing::info!(
        target: TARGET,
        account = %account,
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
            tracing::warn!(target: TARGET, account = %account, schedule = %schedule.id, error = %error, "schedule not recorded");
        }
    }
    core.schedules.changed();
    core.schedules.notify(account, &schedule.id);
}

/// 核心起来时已经过了的一个（错过的不补）：跑完就没了的删掉，她自己定节奏的停着等，别的照现在算下一次；记一行。推给扩展的
/// 记下错过了几次，下一次推的时候报（施工 J-4）。
pub(super) fn skip(table: &Table, account: &AccountId, mut schedule: Schedule, now: Timestamp) {
    let due = schedule.next;
    tracing::info!(target: TARGET, account = %account, schedule = %schedule.id, "schedule missed");
    if let (Target::Package { missed }, Some(due)) = (&mut schedule.target, due) {
        *missed = missed.saturating_add(next::missed(&schedule.when, due, now, &zone()));
    }
    let written = if next::once(&schedule.when) && schedule.looping != Some(Loop::Dynamic) {
        table.remove(&schedule.id).map(|_| ())
    } else {
        schedule.next = match (schedule.looping, due) {
            (Some(Loop::Dynamic), _) | (_, None) => None,
            (_, Some(due)) => next::after(&schedule.when, due, now, &zone()),
        };
        table.put(&schedule).map(|_| ())
    };
    if let Err(error) = written {
        tracing::warn!(target: TARGET, account = %account, schedule = %schedule.id, error = %error, "schedule not advanced");
    }
}
