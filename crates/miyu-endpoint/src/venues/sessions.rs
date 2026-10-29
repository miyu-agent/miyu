//! `venue.sessions {}`（施工 O-32 前，`docs/blueprint/venues.md`「列场所会话」）：这个桥的平台的场所会话，每个场所一个，回应
//! `{"sessions": [{"session", "venue"}]}`。通讯平台的桥起来时照它订阅，不用等每个场所来一条消息才找。
//!
//! 一个场所只看最新的那一个（和 `venue.session` 找回的是同一个；删了的挪进了回收处，本来就不在）；场所编号的平台前缀（第一个
//! `:` 前面那一段）在这个包清单 `[connection] platform` 里的才列（施工 SU-4 中：场所会话都在主人名下，不再照属主分）。平台只认
//! 前缀，核心不解读场所编号别的部分。只给平台桥，别的照没批 `act_for_external` 拒。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;

use super::mains;
use crate::Core;
use crate::hello::Caller;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// `venue.sessions` 的参数：没有格，多写的是参数不对。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionsParams {}

/// `venue.sessions`：见模块的说明。从新到旧（会话编号是 UUIDv7，照编号排就是照造的先后），不分页：一个平台的场所是桥进过的
/// 群和私聊，桥起来时一次要全。
pub(crate) async fn sessions(
    core: &Arc<Core>,
    caller: &Caller,
    _params: SessionsParams,
) -> Result<Value, Refusal> {
    let bridge = super::bridge(core, caller).ok_or_else(super::bridge::refused)?;
    let platforms = bridge.platforms(core);
    // 场所 → 最新的会话：场所会话都在主人名下（施工 SU-4 中）。
    let mut newest: BTreeMap<String, SessionId> = BTreeMap::new();
    for listed in mains(core, |row| row.venue != LOCAL, None).await? {
        let newer = newest
            .get(&listed.venue)
            .is_none_or(|found| listed.id.as_str() > found.as_str());
        if newer {
            newest.insert(listed.venue, listed.id);
        }
    }
    let mut mine: Vec<(SessionId, String)> = newest
        .into_iter()
        .filter(|(venue, _)| platforms.holds(venue))
        .map(|(venue, session)| (session, venue))
        .collect();
    mine.sort_by(|a, b| b.0.as_str().cmp(a.0.as_str()));
    let sessions: Vec<Value> = mine
        .iter()
        .map(|(session, venue)| json!({"session": session.as_str(), "venue": venue}))
        .collect();
    Ok(json!({"sessions": sessions}))
}

/// 一个桥接的平台（施工 IM-3 起能写几个；SU-4 中起碰得到哪些场所会话只照它）：包清单 `[connection] platform` 写的。
pub(crate) struct Platforms(BTreeSet<String>);

/// 包 `package` 清单 `[connection] platform` 写的平台；没写的、读不成的是空的。
pub(crate) fn platforms_of_package(core: &Core, package: &str) -> Platforms {
    Platforms(
        core.system_packages()
            .iter()
            .find(|found| found.id == package)
            .and_then(|found| found.read.as_ref().ok())
            .and_then(|manifest| manifest.connection.as_ref())
            .map(|connection| connection.platforms.iter().cloned().collect())
            .unwrap_or_default(),
    )
}

impl Platforms {
    /// 一个平台都没写。
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 写了的平台，照名字排。
    pub(crate) fn iter(&self) -> impl Iterator<Item = &String> {
        self.0.iter()
    }

    /// 场所 `venue` 是这几个平台的：平台前缀在里面；本机的（`local` 没有 `:`）、没有前缀的都不是。
    pub(crate) fn holds(&self, venue: &str) -> bool {
        platform_of(venue).is_some_and(|platform| self.0.contains(platform))
    }
}

/// 场所编号、平台身份的平台前缀：第一个 `:` 前面那一段；没有 `:` 的是空的。
fn platform_of(text: &str) -> Option<&str> {
    text.split_once(':').map(|(prefix, _)| prefix)
}

#[cfg(test)]
mod tests;
