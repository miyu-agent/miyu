//! 环境那几块能关（施工 C-11，2026-10-23 项目主人定：群聊里用不上的能关掉）：基础系统的七个功能，没有工具，预设照功能开关。
//! 关了的：回合开始的那一块事实不再注入（以前注入过的照旧留在历史里），system 里跟着它的那一行不拼。
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
//!
//! 预设没装这几个功能（以前的快照、以前的程序）的都开着：快照里记的没开的功能里没有它们。

use miyu_kernel::facts::Gates;

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

/// 哪几块给她看。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    /// 回合开始的几块事实。
    pub facts: Gates,
    /// system 里跑在哪个 harness 里那一行。
    pub harness: bool,
    /// system 里本机是什么系统那一行。
    pub machine: bool,
}

impl Default for Shown {
    fn default() -> Shown {
        Shown {
            facts: Gates::default(),
            harness: true,
            machine: true,
        }
    }
}

impl Shown {
    /// 照会话的预设：快照里记的没开的功能（`PresetPin::off`）里有的关掉；没有预设的都开着。
    pub fn of(pin: Option<&PresetPin>) -> Shown {
        let on = |id: &str| !pin.is_some_and(|pin| pin.off.iter().any(|off| off == id));
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
        }
    }
}

#[cfg(test)]
mod tests;
