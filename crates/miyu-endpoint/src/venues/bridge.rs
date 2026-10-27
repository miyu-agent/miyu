//! 连接在场所里替谁做事（接入通讯平台设计 06 第 4.6 节第 2 样，施工 IM-2）：
//!
//! - 声明了系统账号的包（平台桥）：场所会话归它的系统账号；主人的私聊照主人对应表归主人。和以前一样。
//! - 每人一份的包（`[process] scope = "account"`）的那一份：替谁跑就归谁，场所会话一律在他名下。主人对应表是主人的，只在主人
//!   自己那一份里认；成员自己的对应表随 BD-1。
//!
//! 别的连接在场所里什么都做不了（`no_system_account`）。

use miyu_kernel::id::AccountId;

use super::Owners;
use crate::Core;
use crate::hello::Caller;

/// 连接在场所里替谁做事。
pub(crate) enum Bridge {
    /// 平台桥：它的系统账号。
    System(AccountId),
    /// 每人一份的包替 `account` 跑的那一份，包是 `package`。
    Personal {
        /// 替谁跑。
        account: AccountId,
        /// 包的编号。
        package: String,
    },
}

/// 连接 `caller` 在场所里替谁做事：不是平台桥、也不是每人一份的那一份的没有。
pub(crate) fn bridge(core: &Core, caller: &Caller) -> Option<Bridge> {
    if core.is_system(&caller.account) {
        return Some(Bridge::System(caller.account.clone()));
    }
    let package = caller.package.as_deref()?;
    let instance = crate::extensions::Instance::serving(core, package, caller.acting_for());
    instance.account.map(|account| Bridge::Personal {
        account,
        package: package.to_string(),
    })
}

impl Bridge {
    /// 场所会话在谁名下（没对应上主人对应表的那些）。
    pub(crate) fn account(&self) -> &AccountId {
        match self {
            Bridge::System(account) | Bridge::Personal { account, .. } => account,
        }
    }

    /// 它的包清单说的平台、列谁的场所会话（[`Owners`]）。每人一份的不列别人名下的。
    pub(crate) fn owners(&self, core: &Core) -> Owners {
        match self {
            Bridge::System(account) => super::owners(core, account),
            Bridge::Personal { package, .. } => Owners::of_package(core, package),
        }
    }

    /// 主人对应表在这里认不认：平台桥认；每人一份的只在主人自己那一份里认。
    pub(crate) fn binds(&self, core: &Core) -> bool {
        match self {
            Bridge::System(_) => true,
            Bridge::Personal { account, .. } => *account == core.admin,
        }
    }
}

/// 属主是 `owner` 的场所会话里，主人对应表认不认（`session.send` 的 `as` 记成谁）：主人、平台桥名下的认，成员名下的不认（他自己
/// 的对应表随 BD-1）。
pub(crate) fn binds_for(core: &Core, owner: &AccountId) -> bool {
    *owner == core.admin || core.is_system(owner)
}
