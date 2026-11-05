//! 照 `miyu-core` 那样拼整份配置清单的端口（施工 F-5 补；施工 PK-5补 从 `packages_config.rs` 挪来，几份测试共用）：核心自己的
//! 配置项接上包声明的，装卸以后配置清单当场换。

use std::sync::Arc;

use miyu_config::Item;
use miyu_endpoint::Core;
use miyu_endpoint::builtins::{Builtins, Group};
use miyu_session::testkit::Script;
use miyu_store::packages::Found;
use miyu_tool::Catalog;

use super::{Home, TOKEN};

/// 核心自己的配置项。
pub fn core_items() -> Vec<Item> {
    [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat()
}

/// 照 `miyu-core` 那样拼整份清单的端口：核心自己的，接上包声明的。没有内置包的工具。
pub struct Port;

impl Builtins for Port {
    fn tools(&self, _: &[Found]) -> Result<Vec<Group>, String> {
        Ok(Vec::new())
    }

    fn settings(&self, found: &mut [Found]) -> Vec<Item> {
        let packaged = miyu_endpoint::packages::settle(found, &core_items());
        core_items().into_iter().chain(packaged).collect()
    }

    fn embed(&self, _: &[&Found]) -> Option<miyu_session::EmbedSetup> {
        None
    }

    fn script(
        &self,
        _: &miyu_policy::script::ScriptHead,
        _: std::path::PathBuf,
        _: &str,
    ) -> Result<std::sync::Arc<dyn miyu_tool::Tool>, String> {
        Err("no scripts here".to_string())
    }
}

/// 照磁盘上的系统配置起来、装了端口的核心。
pub fn served(home: &Home) -> Arc<Core> {
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        core_items(),
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_builtins(Arc::new(Port)),
    )
}

/// 照 `miyu-core` 起来时那样造的核心（施工 PK-5补）：资源目录是 `resources`，清单读一次，已卸的包留下的设置项的前缀照
/// [`miyu_endpoint::packages::settings_left::left`] 算好交给配置，配置清单照端口拼；重启核心就是再造一个。
pub fn started(home: &Home, resources: &std::path::Path) -> Arc<Core> {
    started_with(home, resources, Vec::new())
}

/// 同 [`started`]，系统加装那一层照 `distro` 这几处（施工 SD-1）。
pub fn started_with(
    home: &Home,
    resources: &std::path::Path,
    distro: Vec<std::path::PathBuf>,
) -> Arc<Core> {
    let resources = miyu_store::resources::ResourceRoot::at(resources);
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, distro.clone());
    let places = miyu_store::packages::Packages::new(&resources, &home.root, distro.clone());
    let left = miyu_endpoint::packages::settings_left::left(&places, &home.root, &found);
    let items = Port.settings(&mut found);
    let config = miyu_endpoint::config::Config::load_leaving(
        &home.root,
        None,
        (items, left),
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        Core::new(
            home.root.clone(),
            resources,
            Arc::new(Script::new([])),
            Catalog::default(),
            None,
            TOKEN.to_string(),
        )
        .with_config(config)
        .with_packages(found)
        .with_distro(distro)
        .with_builtins(Arc::new(Port)),
    )
}
