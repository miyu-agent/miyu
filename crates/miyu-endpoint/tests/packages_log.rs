//! 系统那一层挪到 `system/packages/`（施工 U23 上上，`docs/blueprint/packages.md`「以前的写法」）：核心起来读清单时挪一次，挪了记
//! `INFO packages moved to system`；两边都有的旧的不动、照新位置用，记 `WARN packages left in home`；挪不动的照旧位置读这一次，记
//! `WARN packages not moved to system`。
//!
//! 全局装一个写进内存的订阅者：全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use miyu_endpoint::packages::load;
use miyu_log::{LevelFilter, Memory};
use miyu_store::packages::Found;
use miyu_store::resources::ResourceRoot;

use support::*;

/// 只带一张 `[package]` 的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\n";

/// 读得成的里面有没有 `id`。
fn has_package(found: &[Found], id: &str) -> bool {
    found.iter().any(|one| one.id == id && one.read.is_ok())
}

#[test]
fn moving_the_system_layer_is_logged_and_never_loses_a_package() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let has = |what: &str| memory.lines().iter().any(|line| line.contains(what));
    let resources = ResourceRoot::at(default_resources());

    // 旧位置有、新位置没有：挪过去，照新位置读。
    let home = Home::new();
    home.write("home/alice/packages/kit/package.toml", KIT);
    let found = load(&resources, &home.root, &alice());
    assert!(has_package(&found, "kit"), "挪了照样读得成");
    assert!(
        home.root
            .path()
            .join("system/packages/kit/package.toml")
            .is_file()
    );
    assert!(
        !home.root.path().join("home/alice/packages").exists(),
        "旧位置没了"
    );
    assert!(
        has("INFO  packages packages moved to system"),
        "{:?}",
        memory.lines()
    );

    // 两边都有：旧的不动，照新位置用。
    let home = Home::new();
    home.write("home/alice/packages/old/package.toml", KIT);
    home.write("system/packages/kit/package.toml", KIT);
    let found = load(&resources, &home.root, &alice());
    assert!(has_package(&found, "kit"), "照新位置用");
    assert!(!has_package(&found, "old"), "旧的不读");
    assert!(
        home.root
            .path()
            .join("home/alice/packages/old/package.toml")
            .is_file(),
        "旧的不动"
    );
    assert!(
        has("WARN  packages packages left in home"),
        "{:?}",
        memory.lines()
    );

    // 挪不动（`system` 是个文件顶替权限、跨盘）：旧的原样留着，照旧位置读。
    let home = Home::new();
    home.write("home/alice/packages/kit/package.toml", KIT);
    if home.root.system().is_dir() {
        std::fs::remove_dir_all(home.root.system()).expect("删得了");
    }
    std::fs::write(home.root.system(), "not a dir").expect("写得进");
    let found = load(&resources, &home.root, &alice());
    assert!(has_package(&found, "kit"), "照旧位置读得成");
    assert!(
        home.root
            .path()
            .join("home/alice/packages/kit/package.toml")
            .is_file(),
        "旧的不动"
    );
    assert!(
        has("WARN  packages packages not moved to system error="),
        "{:?}",
        memory.lines()
    );
}
