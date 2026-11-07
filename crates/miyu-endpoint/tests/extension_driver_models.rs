//! 扩展驱动的模型列表（施工 MD-4，设计 66 第四节第 4 条）：真核心、真扩展进程，`model.list` 带 `refresh` 时经扩展的 `driver.models`
//! 拉这一家的列表。列了的模型进列表（`listed` 里有 `provider`），报了窗口的照它；没实现的照拉不到办，不出错，列表里没有。

use serde_json::{Value, json};

use crate::support::extensions::*;
use crate::support::providers::{data, profiles, routed_serving_with};
use crate::support::*;

/// 装包 `ag`：扩展照 `how` 答（`driver:<…>`），声明驱动 `ag`；批了 `model.driver`。`g` 这一家走它。
async fn listed(how: &str) -> Value {
    let home = Home::new();
    let program = Program::new();
    let args = format!("{:?}, {:?}, {:?}", "hello", how, "serve");
    home.write(
        "opt/ag/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Ag\" }}\n\n[command]\nname = \"ag\"\nprogram = \"{}\"\nabout = {{ en = \"A\" }}\n\n[process]\nargs = [{args}]\nstart = \"always\"\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"ag\"\n",
            program.name()
        ),
    );
    approve(&home, "ag", &["model.driver"]);
    home.write("etc/config.toml", "[providers.g]\ndriver = \"ag/ag\"\n");
    let core = routed_serving_with(&home, &[], data(profiles(json!({}))));
    core.start_drivers();
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    until_state(&mut client, "ag", |one| one["state"] == "running").await;
    let reply = client
        .call("l", "model.list", json!({"provider": "g", "refresh": true}))
        .await;
    core.stop_extensions().await;
    reply
}

/// 回应里 `g` 这一家列出的模型：名字、从哪几处列出、窗口和它的来源。
fn models(reply: &Value) -> Vec<(String, Value, Value)> {
    reply["result"]["providers"][0]["models"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|model| {
            (
                model["model"].as_str().unwrap_or_default().to_string(),
                model["listed"].clone(),
                model["facts"]["window"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_extension_lists_its_models() {
    let reply = listed("driver:echo+steady+list").await;
    let found = models(&reply);
    let names: Vec<&str> = found.iter().map(|(name, _, _)| name.as_str()).collect();
    assert_eq!(names, ["ag-lite", "ag-pro"], "{reply}");
    for (name, listed, _) in &found {
        assert!(
            listed
                .as_array()
                .is_some_and(|from| from.contains(&json!("provider"))),
            "{name}：{listed}"
        );
    }
    let (_, _, window) = &found[1];
    assert_eq!(window["value"], 200_000, "{reply}");
    assert_eq!(window["from"], "provider", "{reply}");
}

#[tokio::test]
async fn an_extension_without_a_model_list_is_not_an_error() {
    let reply = listed("driver:echo+steady+none").await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(models(&reply), Vec::new(), "{reply}");
}
