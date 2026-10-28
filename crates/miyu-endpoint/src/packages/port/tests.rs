//! 在会话的预设里打开装上的包，照会话属主（施工 Y-5 下下）：属主是成员的，打开写进他自己那一层的同名覆盖，机主的不碰。会话里装包
//! 一律装进属主自己那一层（施工 U23 下上）：不带程序的装得上，系统那一层的事拒。

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

/// 会话端口上改系统那一层的每一件（施工 Y-5 下下；U23 下上起装、卸进属主自己那一层）：开扩展、带程序的包、属主家目录以外的路径，
/// 属主是成员的拒 `not_permitted`、是机主的拒 `admin_mode_required`，一件一件挡（工具先看一眼再做，只挡看一眼的话直接调做的那一件
/// 就漏了）；不带程序的装进他自己那一层、卸得掉，只读的照旧给。
#[tokio::test]
async fn a_port_installs_into_the_owner_s_own_layer_and_refuses_system_things() {
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
    let bob = AccountId::parse("bob").expect("合写法");
    let port = |owner: &AccountId| Port {
        core: Arc::downgrade(&core),
        owner: owner.clone(),
    };
    let put = |folder: &std::path::Path, text: &str| {
        std::fs::create_dir_all(folder).expect("建得了");
        std::fs::write(folder.join("package.toml"), text).expect("写得了");
    };
    let mine = root.account_dir(&bob).join("workspace/kit");
    put(&mine, "[package]\nname = { en = \"Kit\" }\n");
    let tool = root.account_dir(&bob).join("workspace/tool");
    put(
        &tool,
        "[package]\nname = { en = \"Tool\" }\nprotocol = [1, 1]\n\n[command]\nname = \"tool\"\nprogram = \"miyu-tool\"\nabout = { en = \"Open tool\" }\n\n[ui]\n",
    );
    let outside = dir.join("kit");
    put(&outside, "[package]\nname = { en = \"Kit\" }\n");
    let reason =
        |got: Result<serde_json::Value, PackageRefusal>| got.err().map(|refusal| refusal.reason);
    let member = port(&bob);
    let refused = [
        (
            "inspect elsewhere",
            reason(member.inspect(outside.clone()).await),
        ),
        (
            "install elsewhere",
            reason(member.install(outside.clone(), None).await),
        ),
        (
            "inspect a program",
            reason(member.inspect(tool.clone()).await),
        ),
        (
            "install a program",
            reason(member.install(tool.clone(), None).await),
        ),
        ("enabling", reason(member.enabling("ext".to_string()).await)),
        ("enable", reason(member.enable("ext".to_string()).await)),
    ];
    for (what, got) in refused {
        assert_eq!(got.as_deref(), Some("not_permitted"), "{what}");
    }
    // 主人的装进系统那一层（施工 SU-1，`tests/package_tool.rs` 守着）。
    assert!(member.inspect(mine.clone()).await.is_ok(), "看一眼他自己的");
    let installed = member.install(mine, None).await.expect("装进他自己那一层");
    assert_eq!(installed["layer"], "user");
    assert!(
        root.account_dir(&bob)
            .join("packages/kit/package.toml")
            .is_file()
    );
    assert!(
        !root.system_packages().join("kit").exists(),
        "系统那一层不动"
    );
    assert!(
        reason(
            port(&AccountId::parse("carol").expect("合写法"))
                .remove("kit".to_string())
                .await
        )
        .as_deref()
            == Some("unknown_package"),
        "别人看不到"
    );
    assert!(member.removing("kit".to_string()).await.is_ok());
    assert!(member.remove("kit".to_string()).await.is_ok());
    assert!(
        !root.account_dir(&bob).join("packages/kit").exists(),
        "卸掉了"
    );
    assert!(member.list().await.is_ok(), "列得出");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
