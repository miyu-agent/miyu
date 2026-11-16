//! 她自己定节奏的循环没定下一次就结束（施工 LO-3，项目主人 2026-11-16 定，照 Claude Code「不调就停」）：`/loop <话>`、光打的
//! `/loop` 交进去开的那一轮结束时，那一行的下一次还是空的（她这一轮里没调 `schedule_wakeup`），删掉这一行、推 `removed`，头上
//! 的图标跟着没了。以前那一行停着等她定：图标在、循环其实停了。
//!
//! - 只看循环交进去开的那一轮（`turn.started` 的 `trigger` 是交进去的那一条）：人插的、别的会话发来的开的一轮不算，那一句被并进
//!   别人开的一轮的也不算（那一轮结束就不再盯）。交的那一刻会话正忙的不盯（`fire.rs`）。
//! - 那一轮怎么结束的都算：打断、出错的也删，留着的那一行再也不会到点。
//! - 删在存储那一层一条语句里查（`end_unpaced`）：她这一轮里定了下一次的、人这时停了它的都不删。固定间隔的、人生、定时任务不碰。
//! - 核心在这一轮里停了的不删：那一行照错过的跳过（`run::skip`）。

use std::sync::{Arc, Weak};

use miyu_kernel::event::Body;
use miyu_kernel::id::{Seq, TurnId};
use miyu_session::{Pushed, Subscription};

use super::{Schedules, TARGET};
use crate::Core;

/// 盯着订阅 `pushes`：任务 `id` 交进去的那一条（序号 `said`）开的那一轮结束时，没定下一次的删掉这一行。在自己的任务里等。
pub(super) fn watch(core: &Arc<Core>, id: String, said: Seq, mut pushes: Subscription) {
    let core: Weak<Core> = Arc::downgrade(core);
    tokio::spawn(async move {
        let mut ours: Option<TurnId> = None;
        let mut others: Vec<TurnId> = Vec::new();
        while let Ok(pushed) = pushes.next().await {
            let Pushed::Events(events) = &*pushed else {
                continue;
            };
            for event in events {
                match &event.body {
                    Body::TurnStarted(started) if started.trigger == Some(said) => {
                        ours = event.turn;
                    }
                    Body::TurnStarted(_) => others.extend(event.turn),
                    Body::TurnEnded(_) if ours.is_some() && event.turn == ours => {
                        if let Some(core) = core.upgrade() {
                            core.schedules.end_unpaced(&id);
                        }
                        return;
                    }
                    // 交进去的那一条并进了别人开的一轮：不是它自己那一轮，不再盯。
                    Body::TurnEnded(_)
                        if ours.is_none()
                            && event.turn.is_some_and(|turn| others.contains(&turn)) =>
                    {
                        return;
                    }
                    _ => {}
                }
            }
        }
    });
}

impl Schedules {
    /// 任务 `id` 那一轮结束了：没定下一次的她自己定节奏的循环删掉，叫醒睡着的那个、推 `removed`（同 `/loop off`）。
    fn end_unpaced(&self, id: &str) {
        let Some(table) = self.opened() else {
            return;
        };
        match table.end_unpaced(id) {
            Ok(true) => {
                tracing::info!(target: TARGET, schedule = %id, "loop ended without a next wakeup");
                self.changed();
                self.notify(id);
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(target: TARGET, schedule = %id, error = %error, "loop not ended");
            }
        }
    }
}
