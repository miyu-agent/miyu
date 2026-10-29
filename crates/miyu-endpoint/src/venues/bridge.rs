//! 平台桥（施工 SU-4 中：照包认）：核心拉起的、清单 `[connection] platform` 写了平台的包，场所的事只有它办。
//! 能力 `act_for_external` 照旧由扩展的能力那一道挡（`connection/rights.rs`）；别的连接（人的头、没写平台的包）在场所里什么都
//! 做不了，拒的照没批这个能力的说法。

use miyu_config::package::Capability;
use serde_json::json;

use super::sessions::{Platforms, platforms_of_package};
use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// 平台桥：它是哪个包。
pub(crate) struct Bridge(String);

/// 连接 `caller` 是不是平台桥：核心拉起的、清单写了平台的包。
pub(crate) fn bridge(core: &Core, caller: &Caller) -> Option<Bridge> {
    let package = caller.package.as_ref()?;
    (!platforms_of_package(core, package).is_empty()).then(|| Bridge(package.clone()))
}

/// 不是平台桥的连接办场所的事：照没批 `act_for_external` 拒（施工 SU-4 中）。
pub(crate) fn refused() -> Refusal {
    Refusal::needs_approval(json!([Capability::ActForExternal.as_str()]))
}

impl Bridge {
    /// 它的包清单写的平台：碰得到这几个平台的场所会话。
    pub(crate) fn platforms(&self, core: &Core) -> Platforms {
        platforms_of_package(core, &self.0)
    }
}
