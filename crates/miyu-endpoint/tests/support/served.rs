//! 照对应表、读好的清单起来、拉起扩展的核心，和等扩展记下回应的（施工 IM-3 从 `system_account.rs` 挪出来：那边到了行数上限）。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::extensions::{quick, read};
use super::venues::BINDINGS;
use super::{Home, TOKEN, alice, default_resources};

/// 照对应表、读好的清单起来的核心，拉起开着的扩展。
pub fn served_core(home: &Home, script: &Script) -> Arc<Core> {
    served_core_with(home, script, BINDINGS)
}

/// 同 [`served_core`]，系统配置照 `config` 写（施工 IM-3：另加别的平台的对应表）。
pub fn served_core_with(home: &Home, script: &Script, config: &str) -> Arc<Core> {
    home.write("config.toml", config);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root);
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 等到记下的回应有 `n` 行（`cwd:` 那一行不算），交回它们。
pub async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
