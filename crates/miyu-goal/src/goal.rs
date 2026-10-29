//! `goal`（施工 X-8 上，`docs/blueprint/tools/goal.md`）：看、设、结束这个会话的长期目标。访问类别是读：只改这个会话自己的
//! 目标，只读开着也能用。
//!
//! - `get`：现在的目标，第几轮，卡住的带原因；没有的说没有。原文照模板的转义换进去（引号、换行写成转义）。
//! - `set`：设一个，当场算授权过，这一轮结束、会话空着就开续轮（`goal.md`「定了的」第 1 条）。已经有一个的拒，说只有人能改。
//! - `complete`、`blocked`：结束。续轮开的那一轮里报的，结果带收尾的说法（`wrapup-*.txt`，照搬旧版），她接着在同一轮写收尾
//!   给人；人开的那一轮只说记下了（旧版同样只在自动轮里收尾）。续轮里报卡住太早的，端口拒，说这是第几轮、至少几轮。

use std::num::NonZeroU32;
use std::path::Path;

use serde::Deserialize;

use miyu_kernel::session::Reason;
use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{
    BLOCKED_AFTER, Call, Done, GOAL, GoalAsk, GoalNow, GoalRefused, GoalSeen, Progress, Running,
    Spec, Tool,
};

use crate::SOFTWARE;
use crate::texts::{Texts, line, said};

/// `goal`。
pub(crate) struct Goal {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数：照 `action` 分，别的参数不认，也不报错。
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Args {
    Get,
    Set {
        objective: String,
        #[serde(default)]
        max_rounds: Option<NonZeroU32>,
    },
    Complete,
    Blocked {
        reason: String,
    },
}

impl Goal {
    /// 照资源目录 `resources` 里的字造。
    pub(crate) fn load(resources: &Path) -> Result<Goal, LoadError> {
        Ok(Goal {
            spec: load::spec(resources, SOFTWARE, GOAL, Access::Read)?,
            texts: Texts::load(resources)?,
        })
    }
}

impl Tool for Goal {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.goals.as_ref() else {
                return texts.off();
            };
            let ask = match args {
                Args::Get => GoalAsk::Get,
                Args::Set {
                    objective,
                    max_rounds,
                } => GoalAsk::Set {
                    text: objective,
                    limit: max_rounds,
                },
                Args::Complete => GoalAsk::Complete,
                Args::Blocked { reason } => GoalAsk::Blocked { reason },
            };
            let seen = match port.goal(ask.clone()).await {
                Ok(seen) => seen,
                Err(refused) => return self.refused(refused),
            };
            match ask {
                GoalAsk::Get => now(texts, &seen),
                GoalAsk::Set { text, .. } => {
                    Done::ok(line(&texts.set, &[])).said(said("goal/set", &[("objective", &text)]))
                }
                GoalAsk::Complete => completed(texts, &seen),
                GoalAsk::Blocked { reason } => blocked(texts, &seen, &reason),
            }
        })
    }
}

impl Goal {
    /// 端口拒了：照哪一种说一句。
    fn refused(&self, refused: GoalRefused) -> Done {
        let texts = &self.texts;
        match refused {
            GoalRefused::TooEarly { round } => {
                let (round, least) = (round.to_string(), BLOCKED_AFTER.to_string());
                let fields = [("round", round.as_str()), ("least", least.as_str())];
                Texts::error(&texts.too_early, "goal/too-early", &fields)
            }
            GoalRefused::Kernel(Reason::GoalExists) => {
                Texts::error(&texts.exists, "goal/exists", &[])
            }
            GoalRefused::Kernel(Reason::NoGoal) => Texts::error(&texts.none, "goal/none", &[]),
            GoalRefused::Kernel(reason) => {
                Texts::error(&texts.failed, "goal/failed", &[("reason", reason.code())])
            }
            GoalRefused::Stopped => {
                Texts::error(&texts.failed, "goal/failed", &[("reason", "stopped")])
            }
        }
    }
}

/// `get`：现在的目标；没有的说没有。
fn now(texts: &Texts, seen: &GoalSeen) -> Done {
    let Some(goal) = &seen.now else {
        return Done::ok(line(&texts.none, &[])).said(said("goal/none", &[]));
    };
    let round = round(goal);
    let fields = [
        ("objective", goal.text.as_str()),
        ("state", goal.state),
        ("round", round.as_str()),
    ];
    let mut text = line(&texts.now, &fields);
    if let Some(reason) = &goal.reason {
        text.push('\n');
        text.push_str(&line(&texts.reason, &[("reason", reason)]));
    }
    Done::ok(text).said(said("goal/now", &[]))
}

/// `complete` 成了：续轮开的那一轮里的带收尾的说法。
fn completed(texts: &Texts, seen: &GoalSeen) -> Done {
    let text = match seen.in_round {
        true => line(&texts.wrapup_completed, &[("objective", objective(seen))]),
        false => line(&texts.completed, &[]),
    };
    Done::ok(text).said(said("goal/completed", &[]))
}

/// `blocked` 成了：同 [`completed`]，收尾的说法里带上卡在哪。
fn blocked(texts: &Texts, seen: &GoalSeen, reason: &str) -> Done {
    let text = match seen.in_round {
        true => line(
            &texts.wrapup_blocked,
            &[("objective", objective(seen)), ("reason", reason)],
        ),
        false => line(&texts.blocked, &[]),
    };
    Done::ok(text).said(said("goal/blocked", &[("reason", reason)]))
}

/// 结束之前的那个目标的原文：换进模板时照转义（`miyu_kernel::template`），引号、换行写不进一行字以外的东西。
fn objective(seen: &GoalSeen) -> &str {
    seen.now.as_ref().map_or("", |goal| goal.text.as_str())
}

/// 第几轮：有上限的写「第几轮，共几轮」，和续轮消息一个写法。
fn round(goal: &GoalNow) -> String {
    match goal.limit {
        Some(limit) => format!("Round {} of {limit}", goal.round),
        None => format!("Round {}", goal.round),
    }
}
