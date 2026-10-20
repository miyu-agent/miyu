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
