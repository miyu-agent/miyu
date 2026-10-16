//! 装卸以后再建系统账号（施工 F-5 下）：开过索引的账号不再开，手里的还是同一份。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[tokio::test]
async fn an_opened_index_is_kept_when_prepared_again() {
    let (root, dir) = temp_root("accounts");
    // 声明了系统账号的包用测试自己装的夹具（施工 S-6 下：出厂的接入QQ 搬出了核心仓库）。
    let alice = AccountId::parse("alice").expect("合写法");
    let package = root.account_dir(&alice).join("packages/courier");
    std::fs::create_dir_all(&package).expect("建得了");
    std::fs::write(
        package.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { en = \"Courier\" }\n\n[command]\nname = \"courier\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"C\" }\n\n[process]\nsystem_account = true\n",
    )
    .expect("写得进");
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        alice,
        "token".to_string(),
    );
    let courier = AccountId::parse("courier").expect("合写法");
    super::prepare(&core);
    let first = core.index_for(&courier).expect("装着的包有系统账号");
    super::prepare(&core);
    let again = core.index_for(&courier).expect("还在");
    assert!(Arc::ptr_eq(&first, &again), "开过的不再开");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
