//! `venue.plugins {venue, on}`（施工 O-60，`docs/blueprint/venues.md`「场所插件」）：通讯平台的桥说一个场所开着哪些场所插件，
//! 整份替换，空的是全不给。核心在内存里记（[`miyu_session::VenuePlugins`]），那个场所的会话下一个回合开始照它重筛工具面；
//! 这一轮里刚关掉的再调回「暂时不可用」。核心、桥刚重启、桥还没说过的场所一件都不给：桥连上以后对每个场所发一次。
//!
//! 只给平台桥，只管它自己平台的场所：场所编号的平台前缀要是它包清单 `[connection] platform` 写的（同 `venue.sessions`）。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::VenueId;

use crate::Core;
use crate::hello::Caller;
use crate::provide::plugin_name;
use crate::refusal::Refusal;

/// `venue.plugins` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginsParams {
    venue: String,
    /// 开着的场所插件的名字（写法同 `provide` 的 `plugin`）；不写的当空的。
    #[serde(default)]
    on: Vec<String>,
}

/// `venue.plugins`：见模块的说明。回应 `{}`。
pub(crate) fn plugins(
    core: &Arc<Core>,
    caller: &Caller,
    params: PluginsParams,
) -> Result<Value, Refusal> {
    let bridge = super::bridge(core, caller).ok_or_else(super::bridge::refused)?;
    let venue = VenueId::parse(&params.venue).map_err(|_| Refusal::BAD_PARAMS)?;
    // 只管自己平台的场所：桥清单没写平台的一个都管不了。
    if !bridge.platforms(core).holds(venue.as_str()) {
        return Err(Refusal::bad_field("venue"));
    }
    if !params.on.iter().all(|name| plugin_name(name)) {
        return Err(Refusal::bad_field("on"));
    }
    let on: BTreeSet<String> = params.on.into_iter().collect();
    core.plugins.set(venue, on);
    Ok(json!({}))
}
