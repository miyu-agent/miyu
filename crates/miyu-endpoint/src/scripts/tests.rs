//! 数据根里的脚本（施工 U23 中上）：挂在 `~home` 下的功能，本机的会话、平台桥的场所会话都有。

use std::sync::Arc;

use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[test]
fn scripts_in_the_data_root_are_installed() {
    let (root, dir) = temp_root("scripts");
    let write = |relative: &str, text: &str| {
        let path = root.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    };
    write(
        "scripts/weather.py",
        "#!/usr/bin/env python3\n# Description: Weather.\nprint('sunny')\n",
    );
    let core = Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        "token".to_string(),
    );
    core.follow_scripts();
    assert!(super::features(&core).installed("script-weather"));
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
