//! 照 `miyu-core` 起来时那样造的核心：清单读一次，照核心自己的几项 `settle`，包的配置项拼进配置清单（施工 9-1 下）。

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_session::testkit::Script;

use super::{Home, TOKEN, default_resources};

/// 一份核心：数据根里这时有的包都读进来，它们的配置项在配置清单里。
pub fn core(home: &Home) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = Config::load(&home.root, None, items, Environment::of(&[]));
    Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_packages(found),
    )
}

/// 装着的样本包的编号：要「下面一层」的测试把人格、预设放进它（施工 SU-3 下：下面一层只有出厂的、包带的）。
pub const SAMPLES: &str = "samples";

/// 写一个装着的样本包 [`SAMPLES`]（只有清单），交回它的文件夹；人格、预设照 `personas/<编号>/`、`presets/<编号>.toml` 往里放。
/// 用 [`core`] 起的核心才读得到它。
pub fn samples(home: &Home) -> std::path::PathBuf {
    home.write(
        &format!("packages/{SAMPLES}/package.toml"),
        "[package]\nname = { en = \"Samples\" }\n",
    );
    home.root.path().join("packages").join(SAMPLES)
}

/// 样本人格从数据根挪进样本包：`engineer` 成了下面那一层，家目录里没有它。
pub fn sample_persona_below(home: &Home) {
    let dir = samples(home).join("personas");
    std::fs::create_dir_all(&dir).expect("建得了");
    let id = miyu_session::testkit::SAMPLE_PERSONA;
    std::fs::rename(home.root.path().join("personas").join(id), dir.join(id)).expect("挪得了");
}
