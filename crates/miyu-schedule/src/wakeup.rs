//! `schedule_wakeup`（施工 J-2，设计 45 第三节第 4 条，照 Claude Code 的 `ScheduleWakeup`）：没写间隔的 `/loop` 是动态循环，她每轮
//! 结束调它定下一次多久以后醒。访问类别是读。
//!
//! - 间隔照 [`WAKEUP_MIN`]～[`WAKEUP_MAX`] 收边界，超了照边界，不报错；`prompt` 覆盖醒来交进来的话，不写的照旧（照 Claude Code，
//!   她每次把同一句原话传回来）；`reason` 只给人看。成了说下一次几点，照会话的时区写。
//! - `stop: true` 停掉这个会话的动态循环，别的参数不看。
//! - 这个会话没开过循环的，端口拒，说没有在跑的循环。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::template::Template;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{
    Call, Done, Progress, Running, SCHEDULE_WAKEUP, ScheduleAnswer, ScheduleAsk, ScheduleRefused,
    Spec, Tool, WAKEUP_MAX, WAKEUP_MIN,
};

use crate::LOOP;
use crate::texts::{Common, Package, clock};

/// 这个包。
const PACKAGE: Package = Package { id: LOOP };

/// `schedule_wakeup`。
pub(crate) struct Wakeup {
    spec: Spec,
    common: Common,
    set: Template,
    stopped: Template,
    no_loop: Template,
    no_delay: Template,
}

/// 她给的参数：都能不写，`stop` 以外要有 `delay_seconds`。间隔收成整数以前不挑大小：负的、太大的照边界。
#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    delay_seconds: Option<i64>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    stop: bool,
}

impl Wakeup {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<Wakeup, LoadError> {
        let wakeup =
            |name: &str, fields: &[&str]| PACKAGE.text(resources, ("wakeup", name), fields);
        Ok(Wakeup {
            spec: load::spec(resources, LOOP, SCHEDULE_WAKEUP, Access::Read)?,
            common: Common::load(resources, PACKAGE)?,
            set: wakeup("set", &["next"])?,
            stopped: wakeup("stopped", &[])?,
            no_loop: wakeup("no-loop", &[])?,
            no_delay: wakeup("no-delay", &[])?,
        })
    }

    /// 端口办成了。
    fn answered(&self, answer: ScheduleAnswer, offset: UtcOffset, reason: &str) -> Done {
        match answer {
            ScheduleAnswer::Woken(item) => {
                let next = item.next.map_or_else(String::new, |at| clock(at, offset));
                Done::ok(crate::texts::line(&self.set, &[("next", &next)]))
                    .said(PACKAGE.said("wakeup/set", &[("next", &next), ("reason", reason)]))
            }
            ScheduleAnswer::LoopStopped => PACKAGE.ok(&self.stopped, "wakeup/stopped", &[]),
            // 定时任务那几样不是它要的：端口答错了，照用不了说。
            ScheduleAnswer::Created(_) | ScheduleAnswer::Deleted(_) | ScheduleAnswer::Listed(_) => {
                self.common.off()
            }
        }
    }

    /// 端口拒了。
    fn refused(&self, refused: ScheduleRefused) -> Done {
        match refused {
            ScheduleRefused::NoLoop => PACKAGE.error(&self.no_loop, "wakeup/no-loop", &[]),
            ScheduleRefused::Full
            | ScheduleRefused::AllFull
            | ScheduleRefused::BadCron(_)
            | ScheduleRefused::NotFound
            | ScheduleRefused::Unavailable => self.common.off(),
        }
    }
}

impl Tool for Wakeup {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return self.common.bad_args(&error),
            };
            let ask = match (args.stop, args.delay_seconds) {
                (true, _) => ScheduleAsk::StopLoop,
                (false, Some(delay)) => ScheduleAsk::Wakeup {
                    delay: clamp(delay),
                    prompt: args.prompt.filter(|prompt| !prompt.trim().is_empty()),
                },
                (false, None) => return PACKAGE.error(&self.no_delay, "wakeup/no-delay", &[]),
            };
            let (Some(port), Some(ids)) = (call.schedules.as_ref(), call.ids.as_ref()) else {
                return self.common.off();
            };
            let reason = args.reason.unwrap_or_default();
            match port.schedule(&ids.session, ask).await {
                Ok(answer) => self.answered(answer, call.offset, &reason),
                Err(refused) => self.refused(refused),
            }
        })
    }
}

/// 收进 [`WAKEUP_MIN`]～[`WAKEUP_MAX`]。
fn clamp(delay: i64) -> u64 {
    u64::try_from(delay)
        .unwrap_or(0)
        .clamp(WAKEUP_MIN, WAKEUP_MAX)
}
