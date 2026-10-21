//! 这一次谁在听（施工 R-13 上，`docs/blueprint/memory.md` 第九条）：记忆只有一份，AI 的记忆是一个整体，就像人类一样
//! （2026-10-11、2026-10-21 项目主人定）。哪儿都想得起全部（施工 R-29：原来一个场所里只想起在这个场所说过的，2026-10-21
//! 项目主人推翻）；谁在听只管记下时写什么：听众、在哪个场所说的。说不说全凭她自己，不另加规矩。

use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::origin::{By, Person};

/// 这一次谁在听。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listener {
    /// 主人：会话的属主就是记忆归的那个账号（本机的会话、主人自己的私聊、人经协议碰记忆）。里面是记下时写进听众的那个人。
    Owner(By),
    /// 一个场所：属主不是记忆归的那个账号的场所会话（系统账号的群、别人的私聊）。核心不认群还是私聊，场所就是场所。
    Place(VenueId),
}

impl Listener {
    /// 照会话定：属主 `owner` 就是记忆账号 `account` 的，是主人；不是的，是会话所在的场所 `venue`。
    #[must_use]
    pub fn of(account: &AccountId, owner: &AccountId, venue: &VenueId) -> Listener {
        if owner == account {
            Listener::Owner(By::Person(Person::new(owner.clone())))
        } else {
            Listener::Place(venue.clone())
        }
    }

    /// 记下时写什么：听众、在哪个场所说的。主人那边的听众是主人、没有场所；场所里的听众空着、场所是这里。
    pub(super) fn heard(&self) -> (Vec<By>, Option<VenueId>) {
        match self {
            Listener::Owner(by) => (vec![by.clone()], None),
            Listener::Place(venue) => (Vec::new(), Some(venue.clone())),
        }
    }
}
