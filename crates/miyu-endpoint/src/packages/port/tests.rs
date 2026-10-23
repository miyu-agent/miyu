//! 在会话的预设里打开装上的包，照会话属主（施工 Y-5 下下）：属主是成员的，打开写进他自己那一层的同名覆盖，机主的不碰。成员现在
//! 装不了包、从外面走不到这里，所以直接调。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::*;
use crate::test_support::{resources, temp_root};

#[tokio::test]
async fn opening_in_a_preset_writes_the_owner_s_own_layer() {
    let (root, dir) = temp_root("open-in");
    let (alice, bob) = (
        AccountId::parse("alice").expect("合写法"),
        AccountId::parse("bob").expect("合写法"),
    );
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        alice.clone(),
        "token".to_string(),
    ));
    let port = Port {
        core: Arc::downgrade(&core),
        owner: bob.clone(),
    };
    // 出厂的 `dev` 不开没列出来的软件：打开要写进同名覆盖。
    open_in(&core, (&bob, port.peer(&core)), "dev", "kit").await;
    let preset = |account: &AccountId| {
        std::fs::read_to_string(root.account_dir(account).join("presets/dev.toml"))
            .unwrap_or_default()
    };
    assert!(
        preset(&bob).contains("kit = true"),
        "写进属主那一层：{}",
        preset(&bob)
    );
    assert!(
        !preset(&alice).contains("kit"),
        "机主的不碰：{}",
        preset(&alice)
    );
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 会话端口上改机器的每一件（施工 Y-5 下下）：属主是成员的都拒 `not_permitted`，一件一件挡（工具先看一眼再做，只挡看一眼的话
/// 直接调做的那一件就漏了）；只读的照旧给。
#[tokio::test]
async fn a_member_s_port_refuses_every_change_but_still_lists() {
    let (root, dir) = temp_root("port-member");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    ));
    let port = Port {
        core: Arc::downgrade(&core),
        owner: AccountId::parse("bob").expect("合写法"),
    };
    let path = dir.join("kit");
    let refused = [
        ("inspect", port.inspect(path.clone()).await),
        ("install", port.install(path.clone(), None).await),
        ("enabling", port.enabling("ext".to_string()).await),
        ("enable", port.enable("ext".to_string()).await),
        ("removing", port.removing("kit".to_string()).await),
        ("remove", port.remove("kit".to_string()).await),
        ("fetching", port.fetching("kit".to_string()).await),
        ("fetch", port.fetch("kit".to_string(), None).await),
        ("upgrading", port.upgrading(None).await),
        ("upgrade", port.upgrade(None).await),
    ];
    for (what, got) in refused {
        let reason = got.err().map(|refusal| refusal.reason);
        assert_eq!(reason.as_deref(), Some("not_permitted"), "{what}");
    }
    assert!(port.list().await.is_ok(), "列得出");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
