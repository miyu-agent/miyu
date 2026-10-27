//! 自己装（施工 U23 下上，设计 06 第八又补第 5 条）：不在管理员模式里的 `package.install`、`package.remove` 改「替谁干活」那个人
//! 自己那一层（`home/<账号>/packages/`），只对他生效：他的 `package.list` 看得到（`layer: "user"`），同编号的盖过系统那一份，别人
//! 看不到。只收不带程序的；成员从本机路径装的只能是他自己家目录里的；卸只卸自己那一层的。管理员在管理员模式里照旧装进系统那一层。
//! 推送只推给他自己。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;

use crate::support::login::{call_system, member};
use crate::support::*;

/// 只带一张 `[package]` 的包。
const KIT: &str = "[package]\nname = { en = \"Kit\" }\n";

/// 带子命令（有程序）的包。
const TOOL: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Tool\" }\n\n[command]\nname = \"tool\"\nprogram = \"miyu-tool\"\nabout = { en = \"T\" }\n\n[ui]\n";

/// 在 `dir` 里放一个清单写成 `text` 的包 `id`，交回它的文件夹。
fn put(dir: &Path, id: &str, text: &str) -> PathBuf {
    let folder = dir.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    folder
}

fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// `package.list` 里编号是 `id` 的那几项的层。
async fn layers(client: &mut Client, id: &str) -> Vec<String> {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["package"] == id)
        .filter_map(|one| one["layer"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn a_member_installs_into_his_own_layer_and_only_he_sees_it() {
    let home = Home::new();
    home.write("system/packages/kit/package.toml", KIT);
    let core = home.core(&Script::new([]));
    let mut bob = member(&core, "bob").await;
    let mut owner = Client::connect(Arc::clone(&core));
    owner.hello().await;
    let workspace = home
        .root
        .workspace(&AccountId::parse("bob").expect("合写法"));
    let kit = put(&workspace, "kit", KIT);
    let notes = put(&workspace, "notes", KIT);
    let tool = put(&workspace, "tool", TOOL);
    let elsewhere = put(&home.work, "elsewhere", KIT);

    // 看一眼、装：进他自己那一层，系统那一层不动。
    let looked = bob
        .call(
            "p",
            "package.install",
            json!({"path": notes, "preview": true}),
        )
        .await;
    assert_eq!(looked["result"]["package"], "notes", "{looked}");
    let installed = bob
        .call("i", "package.install", json!({"path": notes}))
        .await;
    assert_eq!(installed["result"]["layer"], "user", "{installed}");
    assert!(
        home.root
            .path()
            .join("home/bob/packages/notes/package.toml")
            .is_file()
    );
    assert!(!home.root.path().join("system/packages/notes").exists());
    assert_eq!(layers(&mut bob, "notes").await, ["user"]);
    assert!(layers(&mut owner, "notes").await.is_empty(), "别人看不到");
    let info = bob
        .call("n", "package.info", json!({"package": "notes"}))
        .await;
    assert_eq!(info["result"]["layer"], "user", "{info}");
    assert!(
        info["result"].get("source").is_none(),
        "成员看不到本机路径：{info}"
    );
    assert!(
        info["result"]["installed"].is_string(),
        "记进他那层的本地库：{info}"
    );

    // 同编号的盖过系统那一份，只对他。
    let mine = bob.call("k", "package.install", json!({"path": kit})).await;
    assert_eq!(mine["result"]["layer"], "user", "{mine}");
    assert_eq!(layers(&mut bob, "kit").await, ["user"]);
    assert_eq!(layers(&mut owner, "kit").await, ["home"]);

    // 带程序的、他家目录以外的路径：不收。
    for (what, path) in [("program", &tool), ("elsewhere", &elsewhere)] {
        for preview in [true, false] {
            let reply = bob
                .call(
                    "x",
                    "package.install",
                    json!({"path": path, "preview": preview}),
                )
                .await;
            assert_eq!(
                reason(&reply),
                Some("not_permitted"),
                "{what} {preview}: {reply}"
            );
        }
    }
    assert!(!home.root.path().join("home/bob/packages/tool").exists());
    assert!(
        !home
            .root
            .path()
            .join("home/bob/packages/elsewhere")
            .exists()
    );

    // 卸：只卸他自己那一层的；卸了自己的 kit，系统那一份又露出来。
    let system = bob
        .call("r", "package.remove", json!({"package": "glob"}))
        .await;
    assert_eq!(reason(&system), Some("not_permitted"), "{system}");
    let looked = bob
        .call(
            "r",
            "package.remove",
            json!({"package": "kit", "preview": true}),
        )
        .await;
    assert_eq!(looked["result"]["layer"], "user", "{looked}");
    let removed = bob
        .call("r", "package.remove", json!({"package": "kit"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    assert_eq!(layers(&mut bob, "kit").await, ["home"]);
    assert!(
        home.root
            .path()
            .join("system/packages/kit/package.toml")
            .is_file()
    );
}

/// 管理员不在管理员模式里也是装进自己那一层；进了管理员模式装进系统那一层。带程序的不在管理员模式里拒 `admin_mode_required`。
#[tokio::test]
async fn an_admin_installs_into_the_system_only_in_admin_mode() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut owner = Client::connect(Arc::clone(&core));
    let reply = owner
        .call(
            "h",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "caps": {"input": true, "scope": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
    let kit = put(&home.work, "kit", KIT);
    let tool = put(&home.work, "tool", TOOL);

    let mine = owner
        .call("i", "package.install", json!({"path": kit}))
        .await;
    assert_eq!(mine["result"]["layer"], "user", "{mine}");
    assert!(home.root.path().join("home/alice/packages/kit").is_dir());
    let program = owner
        .call("t", "package.install", json!({"path": tool}))
        .await;
    assert_eq!(reason(&program), Some("admin_mode_required"), "{program}");

    let system = call_system(&mut owner, "s", "package.install", json!({"path": tool})).await;
    assert_eq!(system["result"]["layer"], "home", "{system}");
    assert!(home.root.path().join("system/packages/tool").is_dir());
}

/// 推送（施工 U23 下上）：自己那一层装卸了的只推给他自己的连接；别人订着的不推。
#[tokio::test]
async fn own_changes_are_pushed_only_to_the_account_itself() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    let subscribed = watching
        .call("s", "subscribe", json!({"stream": "packages"}))
        .await;
    assert!(subscribed.get("result").is_some(), "{subscribed}");
    let mut bob = member(&core, "bob").await;
    let workspace = home
        .root
        .workspace(&AccountId::parse("bob").expect("合写法"));
    let his = put(&workspace, "his", KIT);
    let installed = bob.call("i", "package.install", json!({"path": his})).await;
    assert_eq!(installed["result"]["layer"], "user", "{installed}");
    // 机主自己那一层装一个：推得到，前面 bob 的那一条没推过来。
    let kit = put(&home.work, "kit", KIT);
    let mut acting = Client::connect(Arc::clone(&core));
    let reply = acting
        .call(
            "h",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "caps": {"input": true, "scope": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(reply.get("result").is_some(), "{reply}");
    let mine = acting
        .call("i", "package.install", json!({"path": kit}))
        .await;
    assert_eq!(mine["result"]["layer"], "user", "{mine}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = watching.next_within(left).await.expect("等得到推送");
        if message["method"] != "package.changed" {
            continue;
        }
        assert_ne!(
            message["params"]["package"], "his",
            "别人那一层的不推：{message}"
        );
        if message["params"]["package"] == "kit" {
            assert_eq!(message["params"]["entry"]["layer"], "user", "{message}");
            break;
        }
    }
}
