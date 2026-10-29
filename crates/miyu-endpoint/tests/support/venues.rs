//! 场所会话的测试用的（施工 O-3）：照主人对应表起来、带工具的核心；照数据根里的配置起来的核心（施工 P-1 上）；读一个会话的
//! 策略快照（施工 O-13 中）。

use std::path::Path;
use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_policy::Snapshot;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use super::{Home, TOKEN};

/// 配置：`qq:10001` 是主人（施工 SU-1 补：写了非空的值就算，这里写 `admin`），`qq:10003` 没写。
pub const BINDINGS: &str = "[external.bindings]\n\"qq:10001\" = \"admin\"\n";

/// 照对应表起来、工具目录是 `tools` 的核心：先把对应表写进系统配置。
pub fn bound_core(home: &Home, script: &Script, tools: Catalog) -> Arc<Core> {
    home.write("config.toml", BINDINGS);
    configured_core(home, script, tools)
}

/// 照数据根里这时的配置起来的核心（施工 P-1 上起也读 `persona.default`）。
pub fn configured_core(home: &Home, script: &Script, tools: Catalog) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::CommandSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
        miyu_session::loading::ITEMS,
        miyu_session::switches::ITEMS,
        miyu_session::packages_off::ITEMS,
        miyu_session::unsandboxed::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(script, tools, None, TOKEN)
            .with_config(config),
    )
}

/// 目录在 `dir` 的会话的策略快照。
pub fn snapshot(home: &Home, dir: &Path) -> Snapshot {
    let log = read_events(dir).expect("读得了");
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第一条是 session.created");
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}
