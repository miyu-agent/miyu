//! 环境那几块能关（施工 C-11，2026-10-23 项目主人定：群聊里用不上的能关掉）：内置包 `env` 的八个功能（施工 ENV-1 从基础系统挪过来，
//! 加上「用户身份」），没有工具，预设照功能开关。关了的：回合开始的那一块事实不再注入（以前注入过的照旧留在历史里），system 里
//! 跟着它的那一行不拼；「用户身份」关了，造会话时「关于你」那一块不拼。
//!
//! | 功能 | 事实 | system 那一行 |
//! |---|---|---|
//! | `env-time` | 环境那一块的时间、时区 | |
//! | `env-workdir` | 环境那一块的工作目录 | 本机文件写绝对路径（`local-paths-rule.txt`） |
//! | `env-permission` | 权限级别 | 每一级能做什么（`permission-rule.txt`） |
//! | `env-session` | 会话编号 | |
//! | `env-model` | 模型 | |
//! | `env-harness` | | 跑在哪个 harness 里（`agent-version.txt`） |
//! | `env-machine` | | 本机是什么系统（`machine.txt`） |
//! | `env-profile` | | 「关于你」那一块（`profile.txt`，造会话时拼） |
//!
//! 开着的是装着并且没被关的（施工 ENV-1）：快照里记的没开的（`PresetPin::off`）、没装的（`PresetPin::absent`，`env` 包整个关掉、
//! 卸掉时是全部）都不开。以前的快照、以前的程序没记没装的：照都装着，没记在没开的里的都开着，和以前一样。

use miyu_kernel::facts::Gates;

use crate::features::Features;
use crate::preset::PresetPin;

/// 时间和时区。
pub const TIME: &str = "env-time";
/// 工作目录。
pub const WORKDIR: &str = "env-workdir";
/// 权限级别。
pub const PERMISSION: &str = "env-permission";
/// 会话编号。
pub const SESSION: &str = "env-session";
/// 模型。
pub const MODEL: &str = "env-model";
/// 跑在哪个 harness 里、哪一版。
pub const HARNESS: &str = "env-harness";
/// 本机是什么系统。
pub const MACHINE: &str = "env-machine";
/// 「关于你」那一块（施工 ENV-1）。
pub const PROFILE: &str = "env-profile";

/// 这几个功能所在的内置包（施工 ENV-1）：不是必需的，整包关掉就是哪一块都不注入。
pub const PACKAGE: &str = "env";

/// 这几个功能，照上面表里的先后。
pub const ALL: [&str; 8] = [
    TIME, WORKDIR, PERMISSION, SESSION, MODEL, HARNESS, MACHINE, PROFILE,
];

/// 这几个功能里没装的（`features` 里没有的），照 [`ALL`] 的先后：记进快照（`PresetPin::absent`）。
pub fn absent(features: &Features) -> Vec<String> {
    ALL.iter()
        .filter(|id| !features.installed(id))
        .map(|id| (*id).to_string())
        .collect()
}

/// 哪几块给她看。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// 回合开始的几块事实。
    pub facts: Gates,
    /// system 里跑在哪个 harness 里那一行。
    pub harness: bool,
    /// system 里本机是什么系统那一行。
    pub machine: bool,
    /// 「关于你」那一块（施工 ENV-1）。
    pub profile: bool,
}

impl Default for Shown {
    fn default() -> Shown {
        Shown {
            facts: Gates::default(),
            harness: true,
            machine: true,
            profile: true,
        }
    }
}

impl Shown {
    /// 照会话的预设：快照里记的没开的（`PresetPin::off`）、没装的（`PresetPin::absent`，施工 ENV-1）功能里有的关掉；没有预设的
    /// 都开着。
    pub fn of(pin: Option<&PresetPin>) -> Shown {
        let on = |id: &str| {
            !pin.is_some_and(|pin| pin.off.iter().chain(&pin.absent).any(|off| off == id))
        };
        Shown {
            facts: Gates {
                time: on(TIME),
                workdir: on(WORKDIR),
                permission: on(PERMISSION),
                session: on(SESSION),
                model: on(MODEL),
            },
            harness: on(HARNESS),
            machine: on(MACHINE),
            profile: on(PROFILE),
        }
    }
}

#[cfg(test)]
mod tests;
