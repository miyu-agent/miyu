//! 每次新开会话的定时跑完了（施工 J-3 下）：交进去之前先订阅那个会话，等交进去的那句开的一轮结束，给人推一条
//! `schedule.ran`（`notices.rs`）。那一轮被别的消息并进去、会话没了、核心在停的，不推。

use miyu_kernel::event::{Body, EndReason};
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_session::{Pushed, Subscription};
use serde_json::json;

use super::Schedules;

impl Schedules {
    /// 盯着订阅 `pushes`：交进去的那句（序号 `said`）开的那一轮结束时，推 `schedule.ran`。在自己的任务里等。
    pub(super) fn watch_ran(
        &self,
        (schedule, session): (String, SessionId),
        said: Seq,
        mut pushes: Subscription,
    ) {
        let notices = self.notices.clone();
        let clock = std::sync::Arc::clone(&self.clock);
        tokio::spawn(async move {
            let mut turn: Option<TurnId> = None;
            while let Ok(pushed) = pushes.next().await {
                let Pushed::Events(events) = &*pushed else {
                    continue;
                };
                for event in events {
                    match &event.body {
                        Body::TurnStarted(started) if started.trigger == Some(said) => {
                            turn = event.turn;
                        }
                        Body::TurnEnded(ended) if turn.is_some() && event.turn == turn => {
                            let ok = matches!(ended.reason, EndReason::Completed);
                            let more = json!({"schedule": schedule, "session": session, "ok": ok});
                            notices.send(super::notices::RAN, clock.now(), more);
                            return;
                        }
                        _ => {}
                    }
                }
            }
        });
    }
}
