//! 在会话的预设里打开装上的包，照会话属主（施工 Y-5 下下）；属主不是主人的会话装不了包（施工 SU-2 上）。

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

/// 会话端口上装、卸、升级、开扩展（施工 Y-5 下下，SU-2 上起没有自己那一层）：属主不是主人的会话（场所里替外人的）一件一件拒
/// `not_permitted`（工具先看一眼再做，只挡看一眼的话直接调做的那一件就漏了），什么都不动；只读的照旧给。
#[tokio::test]
async fn a_session_not_the_owner_s_cannot_change_packages() {
    let (root, dir) = temp_root("port-member");
    let alice = AccountId::parse("alice").expect("合写法");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        alice.clone(),
        "token".to_string(),
    ));
    let outsider = Port {
        core: Arc::downgrade(&core),
        owner: AccountId::parse("bridge").expect("合写法"),
    };
    let kit = dir.join("kit");
    std::fs::create_dir_all(&kit).expect("建得了");
    std::fs::write(
        kit.join("package.toml"),
        "[package]\nname = { en = \"Kit\" }\n",
    )
    .expect("写得了");
    let reason =
        |got: Result<serde_json::Value, PackageRefusal>| got.err().map(|refusal| refusal.reason);
    let refused = [
        ("inspect", reason(outsider.inspect(kit.clone()).await)),
        ("install", reason(outsider.install(kit.clone(), None).await)),
        (
            "enabling",
            reason(outsider.enabling("ext".to_string()).await),
        ),
        ("enable", reason(outsider.enable("ext".to_string()).await)),
        (
            "removing",
            reason(outsider.removing("kit".to_string()).await),
        ),
        ("remove", reason(outsider.remove("kit".to_string()).await)),
        (
            "fetching",
            reason(outsider.fetching("kit".to_string()).await),
        ),
        (
            "fetch",
            reason(outsider.fetch("kit".to_string(), None).await),
        ),
        ("upgrading", reason(outsider.upgrading(None).await)),
        ("upgrade", reason(outsider.upgrade(None).await)),
    ];
    for (what, got) in refused {
        assert_eq!(got.as_deref(), Some("not_permitted"), "{what}");
    }
    assert!(!root.system_packages().join("kit").exists(), "什么都没装");
    assert!(outsider.list().await.is_ok(), "列得出");
    let owner = Port {
        core: Arc::downgrade(&core),
        owner: alice,
    };
    assert!(owner.inspect(kit.clone()).await.is_ok(), "主人的会话看得了");
    owner
        .install(kit.clone(), None)
        .await
        .expect("主人的会话装得上");
    // 这台机器上的路径只给主人：不是主人的会话列表里没有 `state`、`package.info` 里没有从哪装的。
    let state = |listed: serde_json::Value| {
        listed
            .as_array()
            .expect("是一串")
            .iter()
            .find(|one| one["package"] == "kit")
            .map(|one| one.get("state").is_some())
    };
    assert_eq!(state(owner.list().await.expect("列得出")), Some(true));
    assert_eq!(state(outsider.list().await.expect("列得出")), Some(false));
    let mine = owner.info("kit".to_string()).await.expect("看得了");
    assert!(mine.get("source").is_some(), "{mine}");
    let theirs = outsider.info("kit".to_string()).await.expect("看得了");
    assert!(theirs.get("source").is_none(), "{theirs}");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
