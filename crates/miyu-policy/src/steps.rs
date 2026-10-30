//! 步数上限（施工 AE-1，`docs/blueprint/policy.md`「对外的样子」`step_limit`、`auto_step_limit`）：一个回合最多请求几次模型。
//! 快照里两格：每一轮都照的 `step_limit`，不是人开的回合（定时任务、长期目标的续轮、重启接着干、回报叫醒……）照的 `auto_step_limit`，内核对这种回合取
//! 两者里小的那个（`miyu_kernel::session::Policy::steps_for`）。数照「这是什么会话」给（2026-10-30 项目主人定，照 Claude Code
//! 主对话不设上限、子代理设的分法）：
//!
//! - 有人看着的（终端、网页，`attended` 是真）：人开的回合不限，自己开的回合照「没人看着」那个数；
//! - 没人看着的（通讯平台、定时新开的会话、`miyu ask` 一次性的）：每一轮都照那个数，出厂 [`UNATTENDED_STEPS`]，预设
//!   `[limits] unattended_steps` 可改；
//! - 通用子代理 [`SUBAGENT_STEPS`]、fork 子代理 [`FORK_STEPS`]：出厂常量，不进预设。fork 子代理的快照是父会话那一份，不改它，
//!   载入时换策略里的数（[`StepLimits::applied`]）。

use miyu_kernel::session::Policy;

use crate::Snapshot;

/// 没人看着时一个回合最多请求几次模型（2026-10-30 项目主人定，原提 200）：预设 `[limits] unattended_steps` 没写的照它。
pub const UNATTENDED_STEPS: u32 = 500;

/// 通用子代理一个回合最多请求几次模型（2026-10-30 项目主人定，比 Claude Code 的 500 宽）。
pub const SUBAGENT_STEPS: u32 = 800;

/// fork 子代理一个回合最多请求几次模型（2026-10-30 项目主人定，比 Claude Code 的 200 宽）。
pub const FORK_STEPS: u32 = 500;

/// 会话是哪一种：照它给两个数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watched {
    /// 有人看着的主会话（快照的 `attended` 是真）。
    Attended,
    /// 没人看着的主会话。
    Unattended,
    /// 通用子代理。
    Subagent,
    /// fork 子代理。
    Fork,
}

impl Watched {
    /// 主会话照 `attended` 分。
    pub fn of(attended: bool) -> Watched {
        match attended {
            true => Watched::Attended,
            false => Watched::Unattended,
        }
    }
}

/// 快照里的两格步数上限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepLimits {
    /// 每一轮都照的：快照的 `step_limit`，没有是不限。
    pub every: Option<u32>,
    /// 不是人开的回合照的：快照的 `auto_step_limit`。以前造的快照里没有。
    pub auto: Option<u32>,
}

impl StepLimits {
    /// 这一种会话给的两个数：`preset` 是预设 `[limits] unattended_steps` 写的，没写的照 [`UNATTENDED_STEPS`]；子代理不看它。
    pub fn of(watched: Watched, preset: Option<u32>) -> StepLimits {
        let unattended = preset.unwrap_or(UNATTENDED_STEPS);
        let (every, auto) = match watched {
            Watched::Attended => (None, unattended),
            Watched::Unattended => (Some(unattended), unattended),
            Watched::Subagent => (Some(SUBAGENT_STEPS), SUBAGENT_STEPS),
            Watched::Fork => (Some(FORK_STEPS), FORK_STEPS),
        };
        StepLimits {
            every,
            auto: Some(auto),
        }
    }
}

impl StepLimits {
    /// 照这两个数换掉内核策略里的两格：fork 子代理用（施工 AE-1）。它的快照就是父会话那一份（同一个快照哈希，请求前缀才一样，
    /// `docs/blueprint/agents.md`「一补」第 2 条），快照里的数是父会话的，载入造好策略以后换成 fork 的。
    #[must_use]
    pub fn applied(self, mut policy: Policy) -> Policy {
        policy.step_limit = self.every;
        policy.auto_step_limit = self.auto;
        policy
    }
}

impl Snapshot {
    /// 记下两格步数上限（[`StepLimits::of`]）。只是快照里的两个数，不发给模型：system、工具面一字不变。
    #[must_use]
    pub fn with_steps(mut self, limits: StepLimits) -> Snapshot {
        self.step_limit = limits.every;
        self.auto_step_limit = limits.auto;
        self
    }

    /// 快照里原样的两格：换快照重拼时照旧带过去用。
    pub fn steps(&self) -> StepLimits {
        StepLimits {
            every: self.step_limit,
            auto: self.auto_step_limit,
        }
    }

    /// 造策略用的两个数：以前造的快照没有 `auto_step_limit`（那时 `step_limit` 一律是 `null`），不读成不限，照 `attended`
    /// 读成出厂的（有人看着的 `(不限, 500)`，没人看着的 `(500, 500)`）：老会话没人看着时一样有个头。
    pub(crate) fn step_limits(&self) -> StepLimits {
        match self.auto_step_limit {
            Some(_) => self.steps(),
            None => StepLimits::of(Watched::of(self.attended), None),
        }
    }
}

#[cfg(test)]
mod tests;
