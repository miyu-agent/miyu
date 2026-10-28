//! 连接在场所里替谁做事（施工 O-4 下；SU-2 下起只有平台桥）：声明了系统账号的包（平台桥），场所会话归它的系统账号；主人的私聊
//! 照主人对应表归主人。别的连接在场所里什么都做不了（`no_system_account`）。

use miyu_kernel::id::AccountId;

use super::Owners;
use crate::Core;
use crate::hello::Caller;

/// 平台桥：它的系统账号。
pub(crate) struct Bridge(AccountId);

/// 连接 `caller` 在场所里替谁做事：不是平台桥的没有。
pub(crate) fn bridge(core: &Core, caller: &Caller) -> Option<Bridge> {
    core.is_system(&caller.account)
        .then(|| Bridge(caller.account.clone()))
}

impl Bridge {
    /// 场所会话在谁名下（没对应上主人对应表的那些）。
    pub(crate) fn account(&self) -> &AccountId {
        &self.0
    }

    /// 它的包清单说的平台、列谁的场所会话（[`Owners`]）。
    pub(crate) fn owners(&self, core: &Core) -> Owners {
        super::owners(core, &self.0)
    }
}
