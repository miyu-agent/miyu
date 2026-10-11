//! 这一次谁在听（施工 R-13 上，`docs/blueprint/memory.md` 第九条，`docs/reviews/2026-10-11-群聊里的记忆.md`）：记忆只有一份，
//! 就像一个人只有一个大脑（2026-10-11 项目主人定）；在哪儿冒出来照谁在听。主人那边想得起全部；一个场所里只想起在这个场所
//! 说过的。私事靠这个挡，不交给她临场判断：模型总有说漏的时候，私事漏一次就收不回来。

use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::origin::{By, Person};
use miyu_recall::Entry;

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

    /// 这一条在这里冒不冒得出来：主人那边全部；场所里只有在这个场所说的（以前记的没有场所，只在主人那边）。
    pub(super) fn hears(&self, entry: &Entry) -> bool {
        match self {
            Listener::Owner(_) => true,
            Listener::Place(venue) => entry.venue.as_ref() == Some(venue),
        }
    }

    /// 在场所 `venue`（本机的没有）开的那几轮，这里搜不搜得到（以前的对话，施工 R-13 下）：主人那边全部；场所里只有在这个场所的。
    pub(super) fn hears_place(&self, venue: Option<&str>) -> bool {
        match self {
            Listener::Owner(_) => true,
            Listener::Place(place) => venue == Some(place.as_str()),
        }
    }

    /// 记下时写什么：听众、在哪个场所说的。主人那边的听众是主人、没有场所；场所里的听众空着、场所是这里。
    pub(super) fn heard(&self) -> (Vec<By>, Option<VenueId>) {
        match self {
            Listener::Owner(by) => (vec![by.clone()], None),
            Listener::Place(venue) => (Vec::new(), Some(venue.clone())),
        }
    }

    /// 是不是主人那边（摘要那一行只在这里交，第三条）。
    pub(super) fn is_owner(&self) -> bool {
        matches!(self, Listener::Owner(_))
    }
}
