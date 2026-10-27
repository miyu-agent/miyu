//! 扩展自己退、常驻（施工 IM-1 下，接入通讯平台设计 06 第 4.6 节第 1 样）。
//!
//! - `process.retire {}`：核心这一刻没有发给它、还没回的请求，答 `{"ok": true}`，以后不再往它发（[`crate::reverse::Peer::retire`]）；
//!   有的答 `{"ok": false}`，它过一会儿再问。答应了、退出码 0 的算自己退：记成停下（`retired`），不重启。这一份新来的调用照
//!   「用到时拉起」等它退干净了再起一份（`provide/reach.rs`）。
//! - `process.keep {"on"}`：照（包，账号）记在 `system/kept.json`，核心重启也在；核心起来时每人一份的包只替记着的人拉起。进程
//!   退出不清，只由扩展自己改。一份的包本来就常驻：答 `{"on": true}`，不记。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::kept::{self, Kept};

use super::{Extensions, Instance, TARGET};
use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

impl Extensions {
    /// `instance` 这一次拉起的进程说要退了、核心答应了：退出码 0 的算自己退。
    fn retiring(&self, instance: &Instance) {
        if let Some(slot) = self.slots().get(instance) {
            slot.status.retire();
        }
    }
}

/// `process.retire`：见模块说明。不是核心拉起的扩展的连接 `not_an_extension`。
pub(crate) fn retire(core: &Core, caller: &Caller) -> Result<Value, Refusal> {
    let instance = instance(core, caller)?;
    if !caller.reverse.retire() {
        return Ok(json!({"ok": false}));
    }
    core.extensions.retiring(&instance);
    tracing::info!(target: TARGET, package = instance.label(), "extension retiring");
    Ok(json!({"ok": true}))
}

/// `process.keep` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeepParams {
    on: bool,
}

/// `process.keep`：见模块说明，回 `{"on": <现在的值>}`。写不进的 `internal_error`，不是核心拉起的扩展的连接 `not_an_extension`。
pub(crate) fn keep(core: &Core, caller: &Caller, params: KeepParams) -> Result<Value, Refusal> {
    let Instance {
        package,
        account: Some(account),
    } = instance(core, caller)?
    else {
        return Ok(json!({"on": true}));
    };
    let _one = core
        .extensions
        .kept
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let path = core.root.system().join(kept::FILE);
    let (mut listed, version) = match kept::read(&path) {
        Ok(read) => (read.kept, read.version),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "kept accounts unreadable");
            (Kept::default(), None)
        }
    };
    if listed.on(&package, account.as_str()) != params.on {
        listed.set(&package, account.as_str(), params.on);
        kept::write(&path, &listed, version.as_deref()).map_err(|error| {
            tracing::error!(target: TARGET, package = package.as_str(), error = %error, "kept accounts not written");
            Refusal::INTERNAL
        })?;
    }
    Ok(json!({"on": params.on}))
}

/// 包 `id` 要常驻的账号：读不成的当没有，记一行 `WARN`。
pub(super) fn kept_accounts(core: &Core, id: &str) -> Vec<AccountId> {
    let path = core.root.system().join(kept::FILE);
    match kept::read(&path) {
        Ok(read) => read
            .kept
            .kept
            .get(id)
            .into_iter()
            .flatten()
            .filter_map(|account| AccountId::parse(account).ok())
            .collect(),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "kept accounts unreadable");
            Vec::new()
        }
    }
}

impl Core {
    /// 替要常驻的人拉起包 `id` 每人一份的那几份（核心起来、装上、升级时）：照 `start_for` 那几道查，不能用的不起。
    pub(super) fn start_kept(self: &Arc<Self>, id: &str) {
        for account in kept_accounts(self, id) {
            self.start_for(&Instance {
                package: id.to_string(),
                account: Some(account),
            });
        }
    }
}

/// 连接是哪一份：不是核心拉起的扩展的 `not_an_extension`。
fn instance(core: &Core, caller: &Caller) -> Result<Instance, Refusal> {
    let package = caller.package.as_deref().ok_or(Refusal::NOT_AN_EXTENSION)?;
    Ok(Instance::serving(core, package, caller.acting_for()))
}
