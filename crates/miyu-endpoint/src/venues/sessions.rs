//! `venue.sessions {}`（施工 O-32 前，`docs/blueprint/venues.md`「列场所会话」）：系统账号名下的场所会话，每个场所一个，回应
//! `{"sessions": [{"session", "venue"}]}`。通讯平台的桥起来时照它订阅，不用等每个场所来一条消息才找。
//!
//! 一个场所只看最新的那一个（和 `venue.session` 照「场所加属主」找回的是同一个；删了的挪进了回收处，本来就不在），属主是这两种
//! 的列：调用的系统账号；场所编号的平台前缀（第一个 `:` 前面那一段）是这个包清单 `[connection] platform` 的，属主是对应表里这个
//! 平台的身份对着的本机账号（终端管理员的私聊，2026-10-11 核心定）。别的不列：属主换过、旧的留在系统账号名下的（再来一句找的是
//! 新的那一个），别的平台的，对应表里已经没有这个平台的身份对着它的。平台只认前缀，对应表的键也是这个写法，核心不解读场所编号
//! 别的部分。只给系统账号的连接，别的回 `no_system_account`。
//!
//! 每人一份的包的那一份（施工 IM-2）：列替谁跑的那个人名下、这个包的平台的场所会话（包没写平台的列他名下全部场所会话），不列别人的。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, SessionId};

use super::{Bridge, mains};
use crate::Core;
use crate::hello::Caller;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// `venue.sessions` 的参数：没有格，多写的是参数不对。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionsParams {}

/// `venue.sessions`：见模块的说明。从新到旧（会话编号是 UUIDv7，照编号排就是照造的先后），不分页：一个系统账号名下的场所
/// 是它进过的群和私聊，桥起来时一次要全。
pub(crate) async fn sessions(
    core: &Arc<Core>,
    caller: &Caller,
    _params: SessionsParams,
) -> Result<Value, Refusal> {
    let bridge = super::bridge(core, caller).ok_or(Refusal::NO_SYSTEM_ACCOUNT)?;
    let caller = bridge.account();
    let owners = bridge.owners(core);
    // 场所 → 最新的会话和它的属主：场所会话的属主只会是管理员（对应表里的）或者系统账号，核心认的账号都看；每人一份的只看他自己。
    let accounts = match bridge {
        Bridge::System(_) => core.accounts(),
        Bridge::Personal { .. } => vec![caller.clone()],
    };
    let mut newest: BTreeMap<String, (SessionId, AccountId)> = BTreeMap::new();
    for account in accounts {
        for listed in mains(core, &account, |row| row.venue != LOCAL, None).await? {
            let newer = newest
                .get(&listed.venue)
                .is_none_or(|(found, _)| listed.id.as_str() > found.as_str());
            if newer {
                newest.insert(listed.venue, (listed.id, account.clone()));
            }
        }
    }
    let mut mine: Vec<(SessionId, String)> = newest
        .into_iter()
        .filter(|(venue, (_, owner))| owners.lists(venue, owner, caller))
        .map(|(venue, (session, _))| (session, venue))
        .collect();
    mine.sort_by(|a, b| b.0.as_str().cmp(a.0.as_str()));
    let sessions: Vec<Value> = mine
        .iter()
        .map(|(session, venue)| json!({"session": session.as_str(), "venue": venue}))
        .collect();
    Ok(json!({"sessions": sessions}))
}

/// 系统账号 `caller` 名下以外还列谁的：它的包清单 `[connection] platform` 是哪个平台（没写的是空的，只列它自己名下的），对应表里
/// 这个平台的身份对着的本机账号（不存在的账号不算）。
///
/// 清单照「系统账号的编号就是包的编号」找（施工 O-4 下的约定）：以后系统账号改了名、不再和包同名，这里要跟着改。
pub(crate) struct Owners {
    /// 包清单写的平台（施工 IM-3 起能写几个）。
    platforms: BTreeSet<String>,
    /// 对应表里这几个平台的身份对着的本机账号：（平台，账号）。
    bound: BTreeSet<(String, AccountId)>,
    /// 每人一份的那一份的（施工 IM-2）：他自己名下的也照平台筛。
    personal: bool,
}

/// 照这时的清单、配置算 [`Owners`]。
pub(crate) fn owners(core: &Core, caller: &AccountId) -> Owners {
    let platforms = platforms_of_package(core, caller.as_str());
    let bound = super::owners_ids(core)
        .iter()
        .filter_map(|id| {
            let platform = platform_of(id).filter(|platform| platforms.contains(*platform))?;
            Some((platform.to_string(), core.admin.clone()))
        })
        .collect();
    Owners {
        platforms,
        bound,
        personal: false,
    }
}

/// 包 `package` 清单 `[connection] platform` 写的平台（施工 IM-3 起能写几个）；没写的、读不成的是空的。
pub(crate) fn platforms_of_package(core: &Core, package: &str) -> BTreeSet<String> {
    core.system_packages()
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok())
        .and_then(|manifest| manifest.connection.as_ref())
        .map(|connection| connection.platforms.iter().cloned().collect())
        .unwrap_or_default()
}

impl Owners {
    /// 每人一份的包 `package` 的那一份（施工 IM-2）：平台照它的清单，不列别人名下的。
    pub(crate) fn of_package(core: &Core, package: &str) -> Owners {
        Owners {
            platforms: platforms_of_package(core, package),
            bound: BTreeSet::new(),
            personal: true,
        }
    }

    /// `venue.sessions` 列不列场所 `venue` 最新的、属主是 `owner` 的会话，调用的是 `caller` 名下：他自己名下的列（施工 IM-2：
    /// 他名下的不全是这个包的，包写了平台的只列这几个平台的；平台桥照旧都列），别人名下的照 [`Owners::take`]。
    fn lists(&self, venue: &str, owner: &AccountId, caller: &AccountId) -> bool {
        if owner != caller {
            return self.take(venue, owner);
        }
        !self.personal || self.platforms.is_empty() || self.platform_is(venue)
    }

    /// 场所 `venue` 是不是这个桥的平台的（施工 O-60；施工 IM-3 起是不是其中一个）：平台前缀在它包清单 `[connection] platform`
    /// 里；没写平台的都不是。
    pub(crate) fn platform_is(&self, venue: &str) -> bool {
        platform_of(venue).is_some_and(|platform| self.platforms.contains(platform))
    }

    /// 场所 `venue` 最新的会话属主是 `owner`、不是调用的系统账号的，列不列：平台是它的、属主是对应表里这个平台的身份对着的。
    pub(crate) fn take(&self, venue: &str, owner: &AccountId) -> bool {
        platform_of(venue).is_some_and(|platform| {
            self.platforms.contains(platform)
                && self.bound.contains(&(platform.to_string(), owner.clone()))
        })
    }
}

/// 系统账号 `caller` 的桥碰不碰得到场所 `venue`、属主是 `owner` 的会话（施工 Y-2 上，`connection/checks.rs` 照它放行）：会话不在
/// 本机；平台在桥清单 `[connection] platform` 里，没写的照场所编号自己的前缀；属主是对应表里这个平台的某个身份对着的本机账号
/// （`venue.session` 照同一张表把主人的私聊归主人）。
pub(crate) fn bridged(core: &Core, caller: &AccountId, venue: &str, owner: &AccountId) -> bool {
    if venue == LOCAL {
        return false;
    }
    let Some(platform) = platform_of(venue) else {
        return false;
    };
    let platforms = platforms_of_package(core, caller.as_str());
    (platforms.is_empty() || platforms.contains(platform))
        && *owner == core.admin
        && super::owners_ids(core)
            .iter()
            .any(|id| platform_of(id) == Some(platform))
}

/// 场所编号、平台身份的平台前缀：第一个 `:` 前面那一段；没有 `:` 的是空的。
fn platform_of(text: &str) -> Option<&str> {
    text.split_once(':').map(|(prefix, _)| prefix)
}

#[cfg(test)]
mod tests;
