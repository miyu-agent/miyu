//! `model.list` 只要几个模型（施工 V-6 上，`docs/blueprint/models.md`「协议」`model.list`）：终端的底栏只要现在用的那个，
//! 不用整份。照 `<供应商>/<模型>` 筛，没配的供应商、没有的模型就不在里面；写法不对的 `bad_params`。

use serde_json::json;

use crate::models::{config, data, list, model};
use crate::support::*;

#[tokio::test]
async fn only_the_named_models_are_listed() {
    let home = Home::new();
    home.write("config.toml", &config("https://relay.example.invalid/v1"));
    let reply = list(
        &home,
        &[],
        data(),
        json!({"models": ["deepseek/deepseek-flash", "nobody/x", "newapi/not-there"]}),
    )
    .await;
    let providers = reply["result"]["providers"].as_array().expect("有");
    let ids: Vec<&str> = providers
        .iter()
        .map(|provider| provider["id"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        ids,
        ["deepseek", "newapi"],
        "没配的供应商不列，配了的照先后"
    );
    let deepseek = &providers[0];
    assert_eq!(deepseek["models"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        model(deepseek, "deepseek-flash")["ref"],
        "deepseek/deepseek-flash"
    );
    assert_eq!(
        providers[1]["models"],
        json!([]),
        "配了的供应商、没有的模型：这一家在，模型一个都不列"
    );
    let all = list(&home, &[], data(), json!({})).await;
    let everything = all["result"]["providers"]
        .as_array()
        .and_then(|providers| {
            providers
                .iter()
                .find(|provider| provider["id"] == "deepseek")
        })
        .expect("有 deepseek");
    assert!(
        everything["models"]
            .as_array()
            .is_some_and(|models| models.len() > 1),
        "不写的照旧列全部"
    );
    for bad in [
        json!(["deepseek"]),
        json!(["/x"]),
        json!(["deepseek/"]),
        json!("deepseek/x"),
    ] {
        let reply = list(&home, &[], data(), json!({"models": bad})).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{bad}");
    }
}
