//! 这一次谁在听（施工 R-13 上，`docs/blueprint/memory.md` 第九条）：记忆只有一份，AI 的记忆是一个整体，就像人类一样
//! （2026-10-11、2026-10-21 项目主人定）。哪儿都想得起全部（施工 R-29：原来一个场所里只想起在这个场所说过的，2026-10-21
//! 项目主人推翻）；谁在听只管记下时写什么：听众、在哪个场所说的。说不说全凭她自己，不另加规矩。

use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::origin::{By, Person};

/// 这一次谁在听。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listener {
    /// 主人：没关在沙盒里的会话（本机的会话、主人自己的私聊、人经协议碰记忆）。里面是记下时写进听众的那个人。
    Owner(By),
    /// 一个场所：关在沙盒里的场所会话（群、别人的私聊）。核心不认群还是私聊，场所就是场所。
    Place(VenueId),
}

impl Listener {
    /// 照会话定：关在沙盒里的（`confined`）是会话所在的场所 `venue`；别的是主人 `owner`（施工 SU-4 中：原来照属主是不是记忆
    /// 账号，场所会话都归主人以后只能照关不关）。
    #[must_use]
    pub fn of(confined: bool, owner: &AccountId, venue: &VenueId) -> Listener {
        match confined {
            true => Listener::Place(venue.clone()),
            false => Listener::Owner(By::Person(Person::new(owner.clone()))),
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
