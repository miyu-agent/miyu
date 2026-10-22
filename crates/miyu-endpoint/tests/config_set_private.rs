//! 改私密目录（施工 5-15 下，`docs/blueprint/config.md` 的 `private_too_broad`）：家目录本身或它的上级整条不收，说的是那一个；
//! 别的照收。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 个人设置的文件，照数据根算。
const PERSONAL: &str = "home/alice/settings.toml";

async fn set(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "config.set", params).await
}

/// 私密目录写了家目录本身或它的上级（施工 5-15 下）：整条不收，那一处的问题是 `private_too_broad`，说的是那一个目录。
#[tokio::test]
async fn a_private_dir_as_wide_as_home_is_refused() {
    let home = Home::new();
    let system_home = home.work.join("system-home");
    std::fs::create_dir_all(&system_home).expect("建得了目录");
    let mut client = Client::connect(home.core_with_items(
        &Script::new([]),
        Some(system_home),
        &[],
        miyu_tool::private::ITEMS,
    ));
    client.hello().await;
    let refused = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.private.add", "value": ["~/notes", "~/"]},
        ]}),
    )
    .await;
    assert_eq!(reason(&refused), Some("config_invalid"), "{refused}");
    let problem = &refused["error"]["data"]["problems"][0];
    assert_eq!(problem["code"], "private_too_broad", "{refused}");
    assert_eq!(problem["got"], "~/", "{refused}");
    assert!(!home.root.path().join(PERSONAL).exists(), "什么都没写");
    let fine = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.private.add", "value": ["~/notes"]},
        ]}),
    )
    .await;
    assert!(fine.get("error").is_none(), "{fine}");
    // 写法不对的（施工 5-15 下补）：协议上照旧 `bad_format`，说的话说该怎么写。
    let wrong = set(
        &mut client,
        "c3",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.private.add", "value": ["relative/dir"]},
        ]}),
    )
    .await;
    let problem = &wrong["error"]["data"]["problems"][0];
    assert_eq!(problem["code"], "bad_format", "{wrong}");
    let message = problem["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("relative/dir") && message.contains("~/"),
        "{message}"
    );
}

/// 默认私密目录（施工 5-15 再补，2026-10-21 项目主人验收时定）：`config.schema` 里叫这个名字，选项是出厂清单，带
/// `ticks: "unlisted"`（勾着的是不在值里的）；加的那一项不带。
#[tokio::test]
async fn the_default_private_dirs_are_ticked_when_unlisted() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_items(
        &Script::new([]),
        None,
        &[],
        miyu_tool::private::ITEMS,
    ));
    client.hello().await;
    let reply = client
        .call(
            "s1",
            "config.schema",
            json!({"keys": ["sandbox.private.add", "sandbox.private.remove"]}),
        )
        .await;
    let items = reply["result"]["items"].as_array().expect("有项");
    let remove = items
        .iter()
        .find(|item| item["key"] == "sandbox.private.remove")
        .expect("有去掉的那一项");
    assert_eq!(remove["ticks"], "unlisted", "{remove}");
    assert_eq!(remove["name"], "默认私密目录", "{remove}");
    assert!(
        remove["options"]
            .as_array()
            .is_some_and(|options| !options.is_empty())
    );
    let add = items
        .iter()
        .find(|item| item["key"] == "sandbox.private.add")
        .expect("有加的那一项");
    assert!(add.get("ticks").is_none(), "{add}");
}

/// 受保护的路径（施工 5-17 下）：工作区里的相对路径照收；家目录整个不收，问题是 `readonly_too_broad`；带 `..` 的写法不对，说的话
/// 说该怎么写。`config.schema` 里默认的那一项选项是出厂清单、勾反着画，同私密目录。
#[tokio::test]
async fn readonly_paths_take_workspace_ones_and_refuse_home() {
    let home = Home::new();
    let system_home = home.work.join("system-home");
    std::fs::create_dir_all(&system_home).expect("建得了目录");
    let mut client = Client::connect(home.core_with_items(
        &Script::new([]),
        Some(system_home),
        &[],
        miyu_tool::readonly::ITEMS,
    ));
    client.hello().await;
    let fine = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.readonly.add", "value": ["Makefile", "~/bin"]},
        ]}),
    )
    .await;
    assert!(fine.get("error").is_none(), "{fine}");
    let refused = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.readonly.add", "value": ["Makefile", "~"]},
        ]}),
    )
    .await;
    assert_eq!(reason(&refused), Some("config_invalid"), "{refused}");
    let problem = &refused["error"]["data"]["problems"][0];
    assert_eq!(problem["code"], "readonly_too_broad", "{refused}");
    assert_eq!(problem["got"], "~", "{refused}");
    let wrong = set(
        &mut client,
        "c3",
        json!({"layer": "personal", "changes": [
            {"key": "sandbox.readonly.add", "value": ["../out"]},
        ]}),
    )
    .await;
    let problem = &wrong["error"]["data"]["problems"][0];
    assert_eq!(problem["code"], "bad_format", "{wrong}");
    let message = problem["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("../out") && message.contains("相对路径"),
        "{message}"
    );
    let reply = client
        .call(
            "s1",
            "config.schema",
            json!({"keys": ["sandbox.readonly.remove"]}),
        )
        .await;
    let remove = &reply["result"]["items"][0];
    assert_eq!(remove["ticks"], "unlisted", "{remove}");
    assert_eq!(remove["name"], "默认受保护路径", "{remove}");
    assert_eq!(remove["options"][0]["value"], ".git/hooks", "{remove}");
}
