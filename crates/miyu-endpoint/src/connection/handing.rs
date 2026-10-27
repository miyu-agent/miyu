//! 握手时交给核心拉起的扩展的那份配置（施工 9-4 下下），从 `connection.rs` 挪出来（施工 J-4）。

use std::sync::Arc;

use serde_json::Value;

use crate::Core;
use crate::extensions::Instance;

/// 握手时交给核心拉起的扩展的那份（施工 9-4 下下）：哪一份（施工 IM-1 中下五起带账号）、交出去的键到值、握手时拿的盯配置的
/// 那一头。
pub(super) struct Handing {
    pub(super) instance: Instance,
    pub(super) handed: std::collections::BTreeMap<String, Value>,
    pub(super) current: tokio::sync::watch::Receiver<Arc<crate::config::Config>>,
}

impl Handing {
    /// 照这一刻的配置算包 `instance.package` 的那份，盯配置的那一头记成看过这一份。每人一份的照那个人自己的配置（施工 IM-1
    /// 中：系统配置、他的个人设置合出来的），一份的照机主那一份，和以前一样。
    pub(super) fn now(core: &Core, instance: &Instance) -> Handing {
        let package = instance.package.as_str();
        let mut current = match &instance.account {
            Some(account) => core.mine(account).hub().current(),
            None => core.config_now(),
        };
        let config = Arc::clone(&current.borrow_and_update());
        let handed = crate::extensions::config::own(&config, package);
        tracing::debug!(target: "miyu::endpoint", package, keys = handed.len(), "extension config handed");
        Handing {
            instance: instance.clone(),
            handed,
            current,
        }
    }
}
