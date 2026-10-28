//! 账号只剩主人一个（施工 SU-4 中：包不再各有一个账号，`docs/construction/SU-移除多用户.md` 第二十节）。原来的系统账号、包的
//! 账号（施工 O-4 下、Y-5 中上）拿掉了：核心拉起的扩展也是主人这边的连接，场所会话都在主人名下，平台桥照包认（`venues/bridge.rs`）。
//! 这几处还带着账号参数的，SU-4 下、SU-6 收。

use miyu_kernel::id::AccountId;

use crate::Core;

impl Core {
    /// 场所会话会在谁名下：只有主人。
    pub(crate) fn accounts(&self) -> Vec<AccountId> {
        vec![self.admin.clone()]
    }

    /// 用量照哪几个账号补：只有主人。
    pub(crate) fn usage_accounts(&self) -> Vec<AccountId> {
        self.accounts()
    }

    /// 属主是 `owner` 的会话，记忆归谁：主人（以前的日志里平台桥名下的会话也归主人，搬家前照样对）。
    pub(crate) fn memory_owner(&self, _owner: &AccountId) -> AccountId {
        self.admin.clone()
    }
}
