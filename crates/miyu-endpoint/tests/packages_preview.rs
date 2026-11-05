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
    home.root.packages().join(id).exists()
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
    home.write("opt/pane/package.toml", &pane("1"));
    home.write("etc/config.toml", "[pane]\nport = 9\n");
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let state = home.root.package_state("pane");
    std::fs::create_dir_all(state.join("db")).expect("建得了");
    std::fs::write(state.join("login.json"), "0123456789").expect("写得进");
    std::fs::write(state.join("db/notes.sqlite"), [0u8; 2048]).expect("写得进");
    let outside = home.work.join("big");
    std::fs::write(&outside, [0u8; 4096]).expect("写得进");
    linked(&outside, &state.join("db/big"));
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
    // 状态目录（施工 PK-4）：真实位置、底下普通文件的字节数；链接不跟进去，指到外面那个 4 KiB 的不算。
    assert_eq!(
        result["state"],
        json!({"path": state.to_string_lossy(), "size": 2058}),
        "{shown}"
    );
    assert!(installed(&home, "pane") && state.exists(), "什么都不动");
    let config = std::fs::read_to_string(home.root.config_file());
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
    assert!(
        shipped["result"].get("state").is_none(),
        "没有状态目录的不写这一格：{shipped}"
    );
    // 状态目录那个位置上是普通文件的：不是目录，卸的时候也删不掉，不写这一格。
    std::fs::write(home.root.package_state("miyu-base-mermaid"), "x").expect("写得进");
    let file = call(
        &mut client,
        "package.remove",
        json!({"package": "miyu-base-mermaid", "preview": true}),
    )
    .await;
    assert!(
        file["result"].is_object() && file["result"].get("state").is_none(),
        "是普通文件的不算状态目录：{file}"
    );
}

/// 卸之前说清配置目录（施工 PK-5）：有东西的写 `config: {path, kept}`，不 `purge` 的留（`kept: true`）、`purge` 的删；不在的、
/// 空的不写，`purge` 的也一样；什么都不动。
#[tokio::test]
async fn removing_says_whether_the_config_stays() {
    let home = Home::new();
    home.write("opt/pane/package.toml", &pane("1"));
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let config = home.root.package_config("pane");
    let looked = |purge: bool| json!({"package": "pane", "preview": true, "purge": purge});
    for purge in [false, true] {
        let none = call(&mut client, "package.remove", looked(purge)).await;
        assert!(
            none["result"].is_object() && none["result"].get("config").is_none(),
            "不在的不写：{none}"
        );
    }
    std::fs::create_dir_all(&config).expect("建得了");
    for purge in [false, true] {
        let empty = call(&mut client, "package.remove", looked(purge)).await;
        assert!(
            empty["result"].is_object() && empty["result"].get("config").is_none(),
            "空的不写：{empty}"
        );
    }
    std::fs::write(config.join("settings.toml"), "volume = 3\n").expect("写得进");
    for purge in [false, true] {
        let shown = call(&mut client, "package.remove", looked(purge)).await;
        assert_eq!(
            shown["result"]["config"],
            json!({"path": config.to_string_lossy(), "kept": !purge}),
            "{shown}"
        );
    }
    let plain = call(
        &mut client,
        "package.remove",
        json!({"package": "pane", "preview": true}),
    )
    .await;
    assert_eq!(
        plain["result"]["config"]["kept"], true,
        "不写 purge 的留：{plain}"
    );
    assert!(
        installed(&home, "pane") && config.join("settings.toml").exists(),
        "什么都不动"
    );
}

/// 配置目录本身是个链接（施工 PK-5）：留的是链接，`purge` 删的也只是链接，照样写这一格。
#[cfg(unix)]
#[tokio::test]
async fn a_linked_config_directory_is_still_shown() {
    let home = Home::new();
    home.write("opt/pane/package.toml", &pane("1"));
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let elsewhere = home.work.join("dotfiles");
    std::fs::create_dir_all(&elsewhere).expect("建得了");
    let config = home.root.package_config("pane");
    std::fs::create_dir_all(config.parent().expect("有上一层")).expect("建得了");
    std::os::unix::fs::symlink(&elsewhere, &config).expect("建得了链接");
    let shown = call(
        &mut client,
        "package.remove",
        json!({"package": "pane", "preview": true}),
    )
    .await;
    assert_eq!(
        shown["result"]["config"],
        json!({"path": config.to_string_lossy(), "kept": true}),
        "{shown}"
    );
}

/// 状态目录里放一个指到 `to` 的符号链接 `at`（Windows 上建链接要权限，放一个空目录顶替：同样不该算进大小）。
fn linked(to: &Path, at: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(to, at).expect("建得了链接");
    #[cfg(not(unix))]
    {
        let _ = to;
        std::fs::create_dir_all(at).expect("建得了");
    }
}

/// 状态目录本身是个链接（施工 PK-4）：卸的时候只删链接，指到的不动，大小算 0；链接指到的目录里的东西不算。
#[cfg(unix)]
#[tokio::test]
async fn a_linked_state_directory_counts_as_nothing() {
    let home = Home::new();
    home.write("opt/pane/package.toml", &pane("1"));
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let elsewhere = home.work.join("kept");
    std::fs::create_dir_all(&elsewhere).expect("建得了");
    std::fs::write(elsewhere.join("photo.png"), [0u8; 512]).expect("写得进");
    let state = home.root.package_state("pane");
    std::fs::create_dir_all(state.parent().expect("有上一层")).expect("建得了");
    std::os::unix::fs::symlink(&elsewhere, &state).expect("建得了链接");
    let shown = call(
        &mut client,
        "package.remove",
        json!({"package": "pane", "preview": true}),
    )
    .await;
    assert_eq!(
        shown["result"]["state"],
        json!({"path": state.to_string_lossy(), "size": 0}),
        "{shown}"
    );
}

#[tokio::test]
async fn restoring_a_shipped_package_shows_it() {
    let home = Home::new();
    home.write("etc/removed/miyu-base-net", "");
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
    let removed = home.root.removed_markers().join("miyu-base-net");
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
