//! 长期目标的事件（施工 X-6，`docs/blueprint/goal.md`「在新的架构里怎么放」第 1 条，`03-事件模型.md` 第三节）：人用 `/goal`
//! 交代、她用 `goal` 工具报完成和卡住的那件要做很久的事。目标的真相只在日志里：第几轮、在不在跑都照这几条和回合算
//! （`miyu-session` 的 `goal.rs`）。前后对不对由账本查（`ledger/goal.rs`）。都不进她的上下文：她照续轮消息知道目标（施工 X-8）。

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use crate::id::CommandId;
use crate::text_enum::text_enum;

/// `goal.set`：设一个目标。`by` 是设的人，或者她自己（`goal` 工具那次调用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalSet {
    /// 目标的原文，照写的。
    pub text: String,
    /// 轮数上限：到了算卡住（施工 X-7）。不写的不限。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<NonZeroU32>,
}

/// `goal.changed`：改了原文或上限，写改完的整份：载入时照最后一条就是现在的样子。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalChanged {
    /// 改完的原文。
    pub text: String,
    /// 改完的上限；不限的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<NonZeroU32>,
}

/// `goal.paused`：停下，不再开续轮。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalPaused {}

/// `goal.resumed`：接着跑。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalResumed {}

/// `goal.ended`：目标结束了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalEnded {
    /// 怎么结束的。
    pub outcome: GoalOutcome,
    /// 卡在哪：卡住的才写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

text_enum!(
    /// 目标怎么结束的。不认识的是新版本才有的：原样留着，也算结束了。
    GoalOutcome {
        /// 她报做完了。
        Completed = "completed",
        /// 她报卡住了，或者到了轮数上限（施工 X-7）。
        Blocked = "blocked",
        /// 人清掉了。
        Cleared = "cleared",
    }
);

#[cfg(test)]
mod tests;

/// 续轮消息的命令编号（施工 X-8 补，`protocol.md`「`message.user`」的约定）：`goal-<第几轮>-<毫秒>`。续轮那条 `message.user`
/// 的 `cause` 就是它，头照它知道这是第几轮，不用翻提示词。会话层照它造（[`round_command`]），视图投影照它读（[`round_of`]）。
pub fn round_command(round: u32, unix_millis: i64) -> String {
    format!("goal-{round}-{unix_millis}")
}

/// 照命令编号认续轮的第几轮：不是 [`round_command`] 的写法的没有。
pub fn round_of(cause: &CommandId) -> Option<u32> {
    let rest = cause.as_str().strip_prefix("goal-")?;
    let (round, millis) = rest.split_once('-')?;
    millis.parse::<i64>().ok()?;
    round.parse().ok()
}
