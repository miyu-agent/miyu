//! `package.info` 的 `tools`（施工 PV-1）：装了以后模型会多哪些工具。没装的照软件源索引那一格原样交，装着的、本机的包文件夹照
//! 包里的脚本开头、清单的功能读；没有工具的不出这一格。

use serde_json::{Value, json};

use miyu_http::testkit::Server;

use crate::support::source::{core, entry, keys, serving, signed_entries};
use crate::support::*;

/// 脚本的开头：两种语言的显示名都写了。
const CRACK: &str = "#!/usr/bin/env python3\n# Description: Look up a game.\n# Display name: Game crack status\n# 显示名称: 游戏破解状态查询\nprint(1)\n";

/// 一件脚本工具在回应里的样子。
fn crack() -> Value {
    json!([{"name": "crack_search", "title": {"en": "Game crack status", "zh": "游戏破解状态查询"}, "from": "script"}])
}

/// 工作目录里带一个脚本的包文件夹。
fn folder(home: &Home, id: &str) -> std::path::PathBuf {
    let folder = home.work.join(id);
    std::fs::create_dir_all(folder.join("scripts")).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!("[package]\nversion = \"1.0\"\nname = {{ en = \"{id}\" }}\n"),
    )
    .expect("写得进");
    std::fs::write(folder.join("scripts/crack_search.py"), CRACK).expect("写得进");
    folder
}

async fn info(client: &mut Client, params: Value) -> Value {
    client.call("i", "package.info", params).await
}

#[tokio::test]
async fn a_package_in_the_source_hands_over_the_tools_of_its_entry() {
    let home = Home::new();
    let (secret, trusted) = keys();
    let mut cracked = entry("crack", json!({"en": "Crack"}), json!({}));
    cracked["tools"] = crack();
    let index = signed_entries(
        "2026-11-03T03:00:00Z",
        &[cracked, entry("plain", json!({"en": "Plain"}), json!({}))],
        &secret,
    );
    let server = Server::start(serving(&index)).await;
    let mut client = Client::connect(core(&home, &server.base_url, &[], trusted));
    client.hello().await;
    let got = info(&mut client, json!({"package": "crack", "source": true})).await;
    assert_eq!(got["result"]["tools"], crack(), "{got}");
    let plain = info(&mut client, json!({"package": "plain", "source": true})).await;
    assert!(
        plain["result"].get("tools").is_none(),
        "没有工具的不出：{plain}"
    );
}

#[tokio::test]
async fn an_installed_package_and_a_local_folder_read_their_scripts() {
    let home = Home::new();
    let (_, trusted) = keys();
    let mut client = Client::connect(core(&home, "http://127.0.0.1:9", &[], trusted));
    client.hello().await;
    let looked = folder(&home, "look");
    let got = info(&mut client, json!({"path": looked})).await;
    assert_eq!(got["result"]["tools"], crack(), "看本机的包文件夹：{got}");

    let kit = folder(&home, "kit");
    let installed = client
        .call("p", "package.install", json!({"path": kit}))
        .await;
    assert_eq!(installed["result"]["package"], "kit", "{installed}");
    let got = info(&mut client, json!({"package": "kit", "source": false})).await;
    assert_eq!(got["result"]["tools"], crack(), "装着的：{got}");

    let plain = home.work.join("plain");
    std::fs::create_dir_all(&plain).expect("建得了");
    std::fs::write(
        plain.join("package.toml"),
        "[package]\nversion = \"1.0\"\nname = { en = \"Plain\" }\n",
    )
    .expect("写得进");
    let got = info(&mut client, json!({"path": plain})).await;
    assert!(
        got["result"].get("tools").is_none(),
        "没有工具的不出：{got}"
    );
}
