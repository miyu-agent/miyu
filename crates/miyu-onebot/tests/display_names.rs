//! 桥的工具给人看的名字（施工 S-6 上）：给人看的字挪进包文件夹以后，核心不再读桥的 `human/`；桥照第三方包的路子在 `provide`
//! 里每件带 `display_name`（`{zh, en, ja}`，照包文件夹 `data/human/<语言>.json` 的 `tools.<名字>.name`），核心照它进
//! `human.get`；原来的 `tools.<名字>.subject` 照 `provide` 的 `subject` 带上，核心一起放进 `human.get`。真核心拉起真桥，照两种
//! 语言问 `human.get`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::group::{WAIT, stopped};
use crate::support::ports::on_free_port;
use crate::support::spawning::{bridge_up, cli, ports_config, text};
use crate::support::*;

/// 桥的五件工具。
const TOOLS: [&str; 5] = ["skip_reply", "recall", "mute", "poke", "fetch_media"];

/// 问核心 `human.get`（语言 `language`），等到五件都有了名字：交回 `tools` 那一格里这五件的名字。
async fn names(home: &Home, language: &str) -> Value {
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let got = core
            .call("h", "human.get", json!({"language": language}))
            .await
            .expect("问得到");
        let names: serde_json::Map<String, Value> = TOOLS
            .iter()
            .map(|tool| (tool.to_string(), got["tools"][tool]["name"].clone()))
            .collect();
        if names.values().all(Value::is_string) {
            return Value::Object(names);
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到桥的工具的名字：{}",
            got["tools"]
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn the_cores_human_texts_name_the_bridges_tools() {
    let home = on_free_port(async |listen| {
        let home = Home::spawning(&Script::new([]), &ports_config(listen));
        let started = cli(&home.root, &["start"]).await;
        assert_eq!(started.status.code(), Some(0), "{}", text(&started.stderr));
        bridge_up(&home.root, listen).await?;
        Ok(home)
    })
    .await;
    assert_eq!(
        names(&home, "zh").await,
        json!({"skip_reply": "不说话", "recall": "撤回", "mute": "禁言", "poke": "戳一戳", "fetch_media": "取图和文件"})
    );
    assert_eq!(
        names(&home, "en").await,
        json!({"skip_reply": "Skip reply", "recall": "Recall", "mute": "Mute", "poke": "Poke", "fetch_media": "Fetch media"})
    );
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let got = core
        .call("h", "human.get", json!({"language": "zh"}))
        .await
        .expect("问得到");
    let subjects: Vec<Value> = TOOLS
        .iter()
        .map(|tool| got["tools"][tool]["subject"].clone())
        .collect();
    assert_eq!(
        subjects,
        [
            json!("reason"),
            Value::Null,
            json!("seconds"),
            Value::Null,
            json!("msg")
        ],
        "显示名后面跟哪一个参数：{}",
        got["tools"]
    );
    stopped(home).await;
}
