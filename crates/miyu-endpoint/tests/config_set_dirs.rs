//! 改别的技能目录（施工 PT-2 外，`internal/blueprint/config.md` 的 `bad_format`）：相对路径整条不收，说的是那一个；别的照收。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 配置文件，照数据根算。
const PERSONAL: &str = "etc/config.toml";

async fn set(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "config.set", params).await
}

/// 别的技能目录（施工 PT-2 外）：`~`、`~/…`、绝对路径收，相对路径 `config.set` 整条不收、报 `bad_format`；手改的
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
