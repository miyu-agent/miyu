//! 桥碰得到哪些会话（施工 Y-2 上，SU-1 补）：属主是主人、平台在对应表里写了的才碰得到；别的桥名下的、对应表里没有这个平台的、
//! 本机的会话都碰不到。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::*;
use crate::test_support::{resources, temp_root};

#[test]
fn a_bridge_reaches_only_the_owner_s_sessions_on_a_bound_platform() {
    let (root, dir) = temp_root("bridged");
    let alice = AccountId::parse("alice").expect("合写法");
    std::fs::create_dir_all(root.system()).expect("建得了");
    std::fs::write(
        root.system().join("config.toml"),
        "[external.bindings]\n\"qq:1\" = \"admin\"\n",
    )
    .expect("写得进");
    let config = crate::config::Config::load(
        &root,
        &alice,
        None,
        crate::settings::EXTERNAL_BINDINGS.to_vec(),
        crate::config::Environment::of(&[]),
    );
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        alice.clone(),
        "token".to_string(),
    )
    .with_config(config);
    let bot = AccountId::parse("bot").expect("合写法");
    let other = AccountId::parse("bot2").expect("合写法");
    assert!(bridged(&core, &bot, "qq:group:1", &alice), "主人的");
    assert!(!bridged(&core, &bot, "qq:group:1", &other), "别的桥名下的");
    assert!(
        !bridged(&core, &bot, "tg:group:1", &alice),
        "对应表里没有这个平台的"
    );
    assert!(!bridged(&core, &bot, LOCAL, &alice), "本机的");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
