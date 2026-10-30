//! 换快照重拼时的步数上限（施工 AE-1，`miyu_policy::steps`）。

use miyu_policy::Snapshot;
use miyu_policy::steps::{StepLimits, Watched};

use super::Preset;

/// 主会话照有没有人看着、这一刻预设 `[limits]` 写的重算：改了预设的数，下一个回合换上的快照就是新的数。子代理（`child`）
/// 的不看预设、预设找不回来的，照旧快照 `old` 的。
pub(super) fn restepped(old: &Snapshot, child: bool, preset: &Preset) -> StepLimits {
    let written = match preset {
        Preset::Unknown => return old.steps(),
        Preset::Same(chosen) | Preset::Changed(chosen) => chosen.file.unattended_steps,
        Preset::Nothing | Preset::NoTools { .. } => None,
    };
    match child {
        true => old.steps(),
        false => StepLimits::of(Watched::of(old.attended), written),
    }
}

#[cfg(test)]
mod tests;
