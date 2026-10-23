//! 资源照连接的账号走（施工 Y-2 中上，设计 `06` 八又补「每人一个独立的 Miyu，共用的只有系统」、U2）：人格、预设、技能、脚本、
//! 知识库，写只写进调用者自己的家目录（`home/<账号>/…`），读是自己的加系统层，同名的自己的盖过系统层的。
//!
//! 系统层是数据根里的系统区（[`miyu_store::root::DataRoot::system`]，各样资源一个子目录：`personas/`、`presets/`、`skills/`、
//! `scripts/`），存储层照它叠在自己的那一层下面，有就读、没有就空；不是管理员的家目录：那是主人自己的东西，当系统层会把主人私人
//! 的东西露给成员（2026-10-23 和核心主会话定）。以后拆出独立的系统账号，改 [`scope`] 和 `DataRoot::system` 这两处。写系统层的
//! 判断随 Y-2 下的管理员模式加。

use miyu_kernel::id::AccountId;

use crate::Core;

/// 一个账号看得到的：写只写它自己的，读的时候它自己的盖过系统层的。
#[derive(Debug, Clone)]
pub(crate) struct Scope {
    /// 自己的：家目录 `home/<账号>/`。
    pub(crate) user: AccountId,
}

/// 账号 `caller` 看得到的。系统账号（桥、以后每个带进程的包，U16）没有自己的一层，照它服务的人：这一版是管理员，同
/// `memory_owner`、人格的 keeper 一个口径（核心主会话 10-23 报的：群、陌生人私聊的会话属主是桥的系统账号，照它自己的家目录找，
/// 主人建的人格群里就找不到了）。
pub(crate) fn scope(core: &Core, caller: &AccountId) -> Scope {
    Scope {
        user: core.memory_owner(caller),
    }
}
