//! 辅助请求的总时限（施工 TT-3，2026-11-13 项目主人定，`docs/blueprint/session/actor.md`「7. 请求模型」第 7 条）：起标题、
//! 回顾这类小请求交给端口时起一个定时的任务，到 [`crate::port::ModelPort::aside_limit`]（出厂 180 秒）还没说完的，叫停端口、
//! 照出错交给内核，内核照辅助请求失败的那一条路了结（回顾拒 `recap_failed`，人要的重起标题推 `session.retitle_failed`）。
//! 只管 [`BOUNDED`] 里的用途：提前压的摘要不管（core1 同日定：大上下文、慢模型常超过 3 分钟，卡掉了回合反而多等），以后
//! 新加的辅助请求默认也不管，要管的写进名单。主请求不归这里。
//!
//! 放在 actor 而不在路由：哪种端口都管得到（扩展驱动、测试的剧本），换人、退会话的模型也算在同一个时限里。到时已经说完了、
//! 被同一种用途的下一次换掉了的，照交给端口的那一刻认不出来，不理。

use std::time::Instant;

use miyu_kernel::event::{CallError, ErrorClass, Purpose};
use miyu_kernel::id::Seq;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::super::{Actor, answer, answer_back};
use super::finished;
use crate::port::Back;

/// 受总时限管的辅助请求的用途（施工 TT-3）：起标题、回顾。
const BOUNDED: [Purpose; 2] = [Purpose::Title, Purpose::Recap];

impl Actor {
    /// 辅助请求 `upto`（用途 `purpose`，`asked` 那一刻交给端口）的总时限：到点送回 [`Back::AsideOverdue`]。用途不在
    /// [`BOUNDED`] 里的不计时。
    pub(super) fn overdue(&self, purpose: Purpose, upto: Seq, asked: Instant) {
        if !BOUNDED.contains(&purpose) {
            return;
        }
        let limit = self.model.aside_limit();
        let backs = self.backs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(limit).await;
            answer_back(
                &backs,
                Back::AsideOverdue {
                    purpose,
                    upto,
                    asked,
                },
            );
        });
    }

    /// 到了总时限：那一次还在路上的，叫停端口（之后它什么都不再报）、记一行收场，交回出错的说完了；已经说完、被换掉了的没有。
    /// 错的分类、说法照路由的总超时（`miyu_http` 的 `too_long`）：可重试，「总超时：180 秒还没有说完」。
    pub(in crate::actor) fn aside_overdue(
        &mut self,
        at: Timestamp,
        purpose: Purpose,
        upto: Seq,
        asked: Instant,
    ) -> Option<Input> {
        let k = self.asides.iter().position(|(asking, seen, _, since)| {
            *asking == purpose && *seen == upto && *since == asked
        })?;
        let (_, _, stop, _) = self.asides.remove(k);
        answer(stop, ());
        self.quiet = Instant::now();
        let error = CallError {
            class: ErrorClass::Retryable,
            message: format!(
                "总超时：{} 秒还没有说完",
                self.model.aside_limit().as_secs_f64()
            ),
            status: None,
            retry_after_ms: None,
        };
        finished(
            upto,
            asked,
            None,
            Some(&error),
            &format!("{} ", purpose.as_str()),
        );
        Some(Input::AsideEnded {
            at,
            purpose,
            upto,
            usage: None,
            cost: None,
            error: Some(error),
            sent: None,
        })
    }
}
