//! 改私密目录（施工 5-15 下，`docs/blueprint/config.md` 的 `private_too_broad`）：家目录本身或它的上级整条不收，说的是那一个；
//! 别的照收。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 个人设置的文件，照数据根算。
const PERSONAL: &str = "config.toml";

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
        json!({"layer": "system", "changes": [
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
        json!({"layer": "system", "changes": [
            {"key": "sandbox.private.add", "value": ["~/notes"]},
        ]}),
    )
    .await;
    assert!(fine.get("error").is_none(), "{fine}");
    // 写法不对的（施工 5-15 下补）：协议上照旧 `bad_format`，说的话说该怎么写。
    let wrong = set(
        &mut client,
        "c3",
        json!({"layer": "system", "changes": [
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
        json!({"layer": "system", "changes": [
            {"key": "sandbox.readonly.add", "value": ["Makefile", "~/bin"]},
        ]}),
    )
    .await;
    assert!(fine.get("error").is_none(), "{fine}");
    let refused = set(
        &mut client,
        "c2",
        json!({"layer": "system", "changes": [
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
        json!({"layer": "system", "changes": [
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

/// 别的技能目录（施工 PT-2 外）：写法同私密目录，`~`、`~/…`、绝对路径收，相对路径 `config.set` 整条不收、报 `bad_format`；手改的
/// 文件 `config.check`（`miyu check` 同一条路）也报，不等读的时候。`config.schema` 里出厂的目录表是 `["~/.agents/skills"]`。
#[tokio::test]
async fn other_skill_folders_are_written_from_home_or_absolute() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_items(
        &Script::new([]),
        Some(home.work.join("me")),
        &[],
        miyu_store::skills::external::ITEMS,
    ));
    client.hello().await;
    let schema = client
        .call(
            "s",
            "config.schema",
            json!({"keys": ["skills.external.dirs"]}),
        )
        .await;
    assert_eq!(
        schema["result"]["items"][0]["default"],
        json!(["~/.agents/skills"]),
        "{schema}"
    );
    let wrong = set(
        &mut client,
        "c1",
        json!({"layer": "system", "changes": [
            {"key": "skills.external.dirs", "value": ["~/a", "relative/dir"]},
        ]}),
    )
    .await;
    assert_eq!(reason(&wrong), Some("config_invalid"), "{wrong}");
    let problem = &wrong["error"]["data"]["problems"][0];
    assert_eq!(problem["code"], "bad_format", "{wrong}");
    assert_eq!(problem["got"], "relative/dir", "{wrong}");
    assert!(!home.root.path().join(PERSONAL).exists(), "什么都没写");
    let absolute = home.work.join("shared").to_string_lossy().into_owned();
    let fine = set(
        &mut client,
        "c2",
        json!({"layer": "system", "changes": [
            {"key": "skills.external.enabled", "value": true},
            {"key": "skills.external.dirs", "value": ["~", "~/a", absolute]},
        ]}),
    )
    .await;
    assert!(fine.get("error").is_none(), "{fine}");
    let checked = client
        .call(
            "c3",
            "config.check",
            json!({"layer": "system", "text": "[skills.external]\ndirs = [\"~/a\", \"skills\"]\n"}),
        )
        .await;
    let problems = checked["result"]["problems"].as_array().expect("有问题");
    assert_eq!(problems.len(), 1, "{checked}");
    assert_eq!(problems[0]["code"], "bad_format", "{checked}");
    assert_eq!(problems[0]["key"], "skills.external.dirs", "{checked}");
    assert!(
        problems[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("skills") && message.contains("~/")),
        "{checked}"
    );
}
