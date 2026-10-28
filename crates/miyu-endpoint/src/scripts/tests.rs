//! 主人家目录里的脚本（施工 U23 中上；施工 SU-3 下起没有成员了）：挂在 `~home` 下的功能，主人的会话、包的账号的（平台桥，场所会话
//! 归它，是主人配的）会话都有。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[test]
fn the_owner_s_own_scripts_are_listed_for_the_owner_and_the_bridge() {
    let (root, dir) = temp_root("home-private");
    let write = |relative: &str, text: &str| {
        let path = root.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    };
    write(
        "system/packages/courier/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Courier\" }\n\n[command]\nname = \"courier\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"C\" }\n\n[process]\nsystem_account = true\n",
    );
    write(
        "home/alice/scripts/weather.py",
        "#!/usr/bin/env python3\n# Description: Weather.\nprint('sunny')\n",
    );
    let core = Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    );
    core.follow_scripts();
    let has_weather = |account: &str| {
        super::features(&core, &AccountId::parse(account).expect("合写法"))
            .installed("script-weather")
    };
    assert!(has_weather("alice"), "机主的照旧");
    assert!(has_weather("courier"), "场所会话的属主（平台桥）照旧");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
