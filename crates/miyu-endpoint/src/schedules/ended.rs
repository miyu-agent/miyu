//! 她自己定节奏的循环没定下一次就结束（施工 LO-3，项目主人 2026-11-16 定，照 Claude Code「不调就停」）：`/loop <话>`、光打的
//! `/loop` 交进去开的那一轮结束时，那一行的下一次还是空的（她这一轮里没调 `schedule_wakeup`），删掉这一行、推 `removed`，头上
//! 的图标跟着没了。以前那一行停着等她定：图标在、循环其实停了。
//!
//! - 只看循环交进去开的那一轮（`turn.started` 的 `trigger` 是交进去的那一条）：人插的、别的会话发来的开的一轮不算，那一句被并进
//!   别人开的一轮的也不算（那一轮结束就不再盯）。交的那一刻会话正忙的不盯（`fire.rs`）。
//! - 那一轮怎么结束的都算：打断、出错的也删，留着的那一行再也不会到点。只有一种不算（施工 LO-3补，修 LO-3 带进来的）：照
//!   「重启了」（`restarted`）结束的，核心有计划地停下时记的，再载入时那一轮接着干，她在接着的那一轮里还会定下一次；原来在这里
//!   当场删掉，接着的那一轮里 `schedule_wakeup` 找不到循环、报错，循环断了。
//! - 删在存储那一层一条语句里查（`end_unpaced`）：她这一轮里定了下一次的、人这时停了它的都不删。固定间隔的、人生、定时任务不碰。
//! - 核心起来时（施工 LO-3补，[`boot`]）：「动态、没定下一次、没停着」的那几行是核心在那一轮里停了（有计划地停、被强杀）或者
//!   核心停着时错过了留下的。载入它跑在的会话：正在跑接着干的那一轮的（命令编号照旧是 `schedule/<任务>/…`），照它接着盯；
//!   空着的（被强杀的那一轮不接着干、错过的没有轮）删掉，它们再也不会到点。

use std::sync::{Arc, Weak};

use miyu_kernel::event::{Body, EndReason, Event};
use miyu_kernel::id::{Seq, TurnId};
use miyu_session::{Pushed, Subscription};
use miyu_store::schedules::{Loop, Target};

use super::{Schedules, TARGET};
use crate::Core;

/// 哪一轮是循环自己的。
pub(super) enum Mine {
    /// 交进去的那一条（序号）开的一轮（`turn.started` 的 `trigger`）。
    Said(Seq),
    /// 起来时接着干的那一轮（[`boot`]）：命令编号是这个前缀（`schedule/<任务>/`）开头的。接着干的那一轮由「重启了」那一条
    /// 触发，`trigger` 不是交进去的那一条，命令编号照旧。订阅时它已经开了：看到的头一个结束就是它的。
    Resumed(String),
}

/// 盯着订阅 `pushes`：任务 `id` 的那一轮（`mine`）结束时，没定下一次的删掉这一行。在自己的任务里等。
pub(super) fn watch(core: &Arc<Core>, id: String, mine: Mine, mut pushes: Subscription) {
    let core: Weak<Core> = Arc::downgrade(core);
    tokio::spawn(async move {
        let mut seen = Seen::default();
        while let Ok(pushed) = pushes.next().await {
            let Pushed::Events(events) = &*pushed else {
                continue;
            };
            for event in events {
                let Some(end) = seen.judge(&mine, event) else {
                    continue;
                };
                if end && let Some(core) = core.upgrade() {
                    core.schedules.end_unpaced(&id);
                }
                return;
            }
        }
    });
}

/// 盯的时候看到了哪几轮开始。
#[derive(Default)]
struct Seen {
    /// 循环自己那一轮。
    ours: Option<TurnId>,
    /// 别人开的。
    others: Vec<TurnId>,
}

impl Seen {
    /// 看一条事件：还要接着看的交回空的；不看了的交回要不要查表删掉（真）。
    fn judge(&mut self, mine: &Mine, event: &Event) -> Option<bool> {
        match (&event.body, mine) {
            (Body::TurnStarted(started), Mine::Said(said)) if started.trigger == Some(*said) => {
                self.ours = event.turn;
                None
            }
            (Body::TurnStarted(_), _) => {
                self.others.extend(event.turn);
                None
            }
            // 重启了：再载入时接着干，起来时照 `boot` 接着盯，这里不删。
            (Body::TurnEnded(ended), Mine::Said(_))
                if self.ours.is_some() && event.turn == self.ours =>
            {
                Some(ended.reason != EndReason::Restarted)
            }
            // 交进去的那一条并进了别人开的一轮：不是它自己那一轮，不再盯。
            (Body::TurnEnded(_), Mine::Said(_))
                if self.ours.is_none()
                    && event.turn.is_some_and(|turn| self.others.contains(&turn)) =>
            {
                Some(false)
            }
            // 起来时：头一个结束是接着干的那一轮的。是循环的，照它怎么结束的；不是的，循环那一轮没接着干，删掉。
            (Body::TurnEnded(ended), Mine::Resumed(prefix)) => {
                let resumed = event
                    .cause
                    .as_ref()
                    .is_some_and(|cause| cause.as_str().starts_with(prefix.as_str()));
                Some(!resumed || ended.reason != EndReason::Restarted)
            }
            _ => None,
        }
    }
}

/// 核心起来时（施工 LO-3补，`start_schedules` 跳过错过的以后）：她自己定节奏的循环里没定下一次、没停着的那几行，一行一个任务：
/// 载入跑在的会话，在跑接着干的那一轮的接着盯，空着的、会话没了的删掉。停着的是人停的，留着给人续。
pub(super) fn boot(core: &Arc<Core>) {
    let Some(table) = core.schedules.opened() else {
        return;
    };
    let rows = match table.list() {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "schedules not read");
            return;
        }
    };
    for row in rows
        .into_iter()
        .filter(|row| row.looping == Some(Loop::Dynamic) && row.next.is_none() && !row.paused)
    {
        let core = Arc::clone(core);
        tokio::spawn(async move {
            let Target::Session { session } = &row.target else {
                core.schedules.end_unpaced(&row.id);
                return;
            };
            let Ok(found) = core.sessions.get(&core, session).await else {
                core.schedules.end_unpaced(&row.id);
                return;
            };
            // 先订阅再看忙不忙：看完到订阅之间结束了的，那一轮的结束不漏（空着的当场查表，她定了的不删）。
            let Ok(pushes) = found.handle.subscribe().await else {
                core.schedules.end_unpaced(&row.id);
                return;
            };
            if !found.handle.busy() {
                core.schedules.end_unpaced(&row.id);
                return;
            }
            let mine = Mine::Resumed(format!("schedule/{}/", row.id));
            watch(&core, row.id.clone(), mine, pushes);
        });
    }
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
