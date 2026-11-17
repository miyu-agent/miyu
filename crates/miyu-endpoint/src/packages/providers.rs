//! 包带的供应商交给配置（施工 PV-4，`config/provided.rs`）：核心起来时、装卸以后、开关以后，照这时装着、开着的包（[`Core::packages_of`]）
//! 换配置的包这一层。最终值变了的推 `config.changed`（`via: package`、`layer: system`，不带 `by`）：头照它重读模型列表。

use miyu_config::merge::Resolved;
use miyu_store::packages::Found;

use super::TARGET;
use crate::Core;
use crate::config::hub::{Hub, Pushing};
use crate::config::provided::Declared;
use crate::config::push::Via;

/// 包 `found` 带的供应商，照包的先后、每个包照清单的先后。清单读不成的不算。
pub(crate) fn declared(found: &[Found]) -> Vec<Declared> {
    found
        .iter()
        .filter_map(|one| Some((one, one.read.as_ref().ok()?)))
        .flat_map(|(one, manifest)| {
            manifest
                .providers
                .iter()
                .map(|provider| (one.id.clone(), provider.clone()))
        })
        .collect()
}

/// `before`、`after` 两份最终值里值或来源不一样的真的键，照键名排。
fn changed(before: &Resolved, after: &Resolved) -> Vec<String> {
    let mut keys: Vec<String> = before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(key) != after.get(key))
        .map(str::to_string)
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

impl Core {
    /// 照这时装着、开着的包换配置的包这一层（装卸、开关以后）：变了的交给会话、核心，推 `config.changed`，记一条
    /// `INFO config changed`。
    pub(crate) fn follow_providers(&self) {
        let declared = declared(&self.packages_of());
        let mut config = self.config();
        let before = config.resolved().clone();
        if !config.provide(declared) {
            return;
        }
        let keys = changed(&before, config.resolved());
        if keys.is_empty() {
            crate::config::handout::publish(self, &config, None);
            return;
        }
        tracing::info!(
            target: TARGET,
            via = %Via::Package.as_str(),
            keys = %keys.join(","),
            "provided providers changed"
        );
        let pushing = Pushing {
            layer: miyu_config::Layer::System,
            via: Via::Package,
            by: None,
            keys,
        };
        crate::config::handout::publish(self, &config, Some(pushing));
    }

    /// 造家底时（[`Core::with_packages`]、[`Core::with_config`]）：配置换上这时的包带的供应商，交给会话、核心的那一头跟着换。还没有
    /// 人在等，不推。
    pub(crate) fn provide_from_start(&mut self) {
        let declared = declared(&self.packages_of());
        let config = self
            .config
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if config.provide(declared) {
            self.hub = Hub::new(config);
        }
    }
}
