//! 装卸落在哪一层（施工 U23 下上，设计 06 第八又补第 5 条「系统装、自己装」）：管理员在管理员模式里的请求（[`Peer::admin_mode`]，
//! 没声明认识管理员模式的老头照旧算）改系统那一层，所有人用；别的一律改「替谁干活」那个人自己那一层（`home/<账号>/packages/`），
//! 只对他自己生效。会话里她的 `package` 工具没有管理员模式，一律装进会话属主自己那里（「装成系统的要人进入管理员模式」）。
//!
//! 自己那一层只收不带程序的包（[`miyu_store::packages::runs_program`]）；卸、升级只碰自己那一层装的。系统那一层的事（带程序的包、卸系统装的和出厂的、
//! 装回出厂的、开关扩展）照 [`Scope::system_only`] 拒：管理员 `admin_mode_required`（她照着告诉人进管理员模式），成员
//! `not_permitted`。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_store::packages::{Found, Packages};

use super::packages;
use crate::Core;
use crate::hello::{Caller, Peer};
use crate::refusal::Refusal;
use crate::wire::Request;

/// 改哪一层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    /// 系统那一层（`system/packages/`）。
    System,
    /// 这个账号自己那一层。
    Own(AccountId),
}

impl Scope {
    /// 协议上的一条请求 `request`：在管理员模式里的改系统那一层，别的改 `caller` 替谁干活的那个人自己那一层。
    pub(crate) fn of(peer: Peer, caller: &Caller, request: &Request) -> Scope {
        match peer.admin_mode(request) {
            true => Scope::System,
            false => Scope::Own(caller.acting_for().clone()),
        }
    }

    /// 只有系统那一层做得了的事：是系统那一层的照做；不是的拒 `not_permitted`（施工 SU-1 起没有管理员模式）。
    pub(crate) fn system_only(&self, admin: bool) -> Result<(), Refusal> {
        match (self, admin) {
            (Scope::System, _) => Ok(()),
            (Scope::Own(_), _) => Err(Refusal::NOT_PERMITTED),
        }
    }

    /// 改这一层时照着比的包：系统那一层的是系统那一份；自己那一层的是他看得到的全部（系统那一份接上他那层，他关了的也在）。
    pub(crate) fn installed(&self, core: &Core) -> Arc<Vec<Found>> {
        match self {
            Scope::System => core.system_packages(),
            Scope::Own(account) => core.packages_seen(account),
        }
    }

    /// 这一层的本地库（装的时刻、从哪装的、来源仓库）：系统那一层的在 `state/packages/.local`，自己那一层的在他那层目录下的
    /// `.local`（点开头的不算包）。
    pub(crate) fn local_root(&self, core: &Core) -> PathBuf {
        match self {
            Scope::System => packages(core).local_root(),
            Scope::Own(account) => own_dir(core, account).join(".local"),
        }
    }
}

/// 账号 `account` 自己那一层的目录：`home/<账号>/packages/`。
pub(crate) fn own_dir(core: &Core, account: &AccountId) -> PathBuf {
    core.root.account_dir(account).join("packages")
}

/// 往 `account` 自己那一层装卸之前：他那层不能就是系统那一层（系统那一层挪不成那一次还在机主家目录，[`Packages::of_account`]）；
/// 是的拒 `internal_error`、记一行，下次核心起来挪成了再装。
pub(crate) fn own_apart(core: &Core, account: &AccountId) -> Result<(), Refusal> {
    if packages(core).home_dir() == Some(own_dir(core, account).as_path()) {
        tracing::warn!(target: super::TARGET, account = account.as_str(), "own packages are the system ones until they are moved");
        return Err(Refusal::INTERNAL);
    }
    Ok(())
}

/// 账号 `account` 的三层（[`Packages::of_account`]）。
pub(crate) fn places_of(core: &Core, account: &AccountId) -> Packages {
    Packages::of_account(&core.resources, &core.root, &core.admin, account)
}

#[cfg(test)]
mod tests;
