//! 装、卸之前看一眼（施工 F-8 下补，设计 `31-软件包.md` 第一节、定了的 B）：`package.install`、`package.remove` 带
//! `preview: true` 的照真做之前查的那几样查一遍，交回要做什么，什么都不动。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一个带子命令、后台页、平台、能力的扩展包。
const EXT: &str = "[package]\nprotocol = [1, 1]\nversion = \"3\"\nname = { en = \"Ext\" }\n\n[page]\ndir = \"page\"\n\n[command]\nname = \"ext\"\nprogram = \"miyu-ext\"\nabout = { en = \"X\" }\n\n[process]\nstart = \"manual\"\ncapabilities = [\"network\"]\n\n[connection]\nplatform = \"chat\"\n";

/// 一个界面包，版本 `version`，两项配置。
fn pane(version: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nversion = \"{version}\"\nname = {{ en = \"Ui\" }}\n\n[ui]\n\n[settings.port]\ntype = \"int\"\ndefault = 1\nlayers = [\"system\"]\nname = {{ en = \"Port\" }}\n\n[settings.zone]\ntype = \"text\"\ndefault = \"x\"\nlayers = [\"system\"]\nname = {{ en = \"Zone\" }}\n"
    )
}

/// 工作目录里放一个包文件夹 `id`，清单是 `text`。
fn folder(home: &Home, id: &str, text: &str) -> PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    folder
}

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

fn installed(home: &Home, id: &str) -> bool {
    home.root.path().join("packages").join(id).exists()
}

async fn preview(client: &mut Client, path: &Path) -> Value {
    call(
        client,
        "package.install",
        json!({"path": path, "preview": true}),
    )
    .await
}

#[tokio::test]
async fn installing_shows_what_comes_and_changes_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let ext = folder(&home, "ext", EXT);
    let shown = preview(&mut client, &ext).await;
    let result = &shown["result"];
    assert_eq!(
        (
            result["package"].clone(),
            result["version"].clone(),
            result["program"].clone()
        ),
        (json!("ext"), json!("3"), json!("process")),
        "{shown}"
    );
    assert_eq!(result["command"], "ext");
    assert_eq!(
        (result["page"].clone(), result["connection"].clone()),
        (json!(true), json!("chat"))
    );
    assert_eq!(
        result["capabilities"][0]["id"], "network",
        "照连接的语言说：{shown}"
    );
    assert_eq!(
        (result["files"].clone(), result["size"].clone()),
        (json!(1), json!(EXT.len()))
    );
    assert!(result.get("replaces").is_none(), "没装过的不换下什么");
    assert!(!installed(&home, "ext"), "什么都不动");

    let v1 = folder(&home, "pane", &pane("1"));
    let done = call(&mut client, "package.install", json!({"path": v1})).await;
    assert_eq!(done["result"]["package"], "pane", "{done}");
    std::fs::write(v1.join("package.toml"), pane("2")).expect("写得进");
    let upgrade = preview(&mut client, &v1).await;
    assert_eq!(
        upgrade["result"]["replaces"],
        json!({"version": "1"}),
        "{upgrade}"
    );
    assert_eq!(upgrade["result"]["settings"], 2);

    let bad = folder(&home, "bad", "[package]\n");
    let refused = preview(&mut client, &bad).await;
    assert_eq!(
        refused["error"]["data"]["reason"], "package_invalid",
        "和真装一样拒：{refused}"
    );
    let shipped = folder(&home, "miyu-base-mermaid", &pane("1"));
    let refused = preview(&mut client, &shipped).await;
    assert_eq!(
        refused["error"]["data"]["reason"], "package_exists",
        "{refused}"
    );
}

#[tokio::test]
async fn removing_shows_what_goes_with_it_and_changes_nothing() {
    let home = Home::new();
    home.write("packages/pane/package.toml", &pane("1"));
    home.write("config.toml", "[pane]\nport = 9\n");
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let state = home.root.path().join("state/packages/pane");
    std::fs::create_dir_all(&state).expect("建得了");
    let shown = call(
        &mut client,
        "package.remove",
        json!({"package": "pane", "preview": true}),
    )
    .await;
    let result = &shown["result"];
    assert_eq!(
        (
            result["layer"].clone(),
            result["version"].clone(),
            result["files"].clone()
        ),
        (json!("home"), json!("1"), json!(1)),
        "{shown}"
    );
    assert_eq!(result["settings"], json!(["pane.port"]), "只说写了的");
    assert_eq!(result["state"], true);
    assert!(installed(&home, "pane") && state.exists(), "什么都不动");
    let config = std::fs::read_to_string(home.root.path().join("config.toml"));
    assert!(config.is_ok_and(|text| text.contains("port = 9")));

    let shipped = call(
        &mut client,
        "package.remove",
        json!({"package": "miyu-base-net", "preview": true}),
    )
    .await;
    assert!(
        shipped["result"].is_object() && shipped["result"].get("files").is_none(),
        "出厂的不删文件：{shipped}"
    );
}

#[tokio::test]
async fn restoring_a_shipped_package_shows_it() {
    let home = Home::new();
    home.write("packages/miyu-base-net.removed", "");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let shown = call(
        &mut client,
        "package.install",
        json!({"package": "miyu-base-net", "preview": true}),
    )
    .await;
    assert_eq!(
        (
            shown["result"]["restores"].clone(),
            shown["result"]["program"].clone()
        ),
        (json!(true), json!("builtin")),
        "{shown}"
    );
    let removed = home.root.path().join("packages/miyu-base-net.removed");
    assert!(removed.exists(), "什么都不动");
    let unknown = call(
        &mut client,
        "package.install",
        json!({"package": "miyu-base-memory", "preview": true}),
    )
    .await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unknown_package",
        "没卸过的：{unknown}"
    );
}

/// 只带 MCP 服务的包（施工 X-2）：看一眼的那一份列出服务名，没有程序那一格。
#[tokio::test]
async fn a_bundle_preview_names_its_mcp_servers() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let bundle = folder(
        &home,
        "xgh",
        "[package]\nname = { en = \"G\" }\n\n[[mcp]]\nname = \"xgithub\"\ncommand = [\"npx\", \"server-github\"]\n\n[[mcp]]\nname = \"xfetch\"\ncommand = [\"uvx\", \"mcp-server-fetch\"]\n",
    );
    let shown = preview(&mut client, &bundle).await;
    assert_eq!(
        shown["result"]["mcp"],
        json!(["xgithub", "xfetch"]),
        "{shown}"
    );
    assert!(shown["result"].get("program").is_none(), "{shown}");
}
