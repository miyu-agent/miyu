//! `model.list` 带上约束解码（施工 L-2，`docs/blueprint/models.md`「协议」）：配置里手写标了 `compat.strict_tools` 的那一家
//! 多一格 `strict_tools: true`，没标的没有这一格。

use serde_json::json;

use crate::models::{config, data, list};
use crate::support::*;

#[tokio::test]
async fn a_strict_provider_says_so_in_the_list() {
    let home = Home::new();
    let source = format!(
        "{}\n[providers.newapi.compat]\nstrict_tools = true\n",
        config("https://relay.example.invalid/v1")
    );
    home.write("system/config.toml", &source);
    let reply = list(&home, &[("DEEPSEEK_API_KEY", "sk-1")], data(), json!({})).await;
    let providers = reply["result"]["providers"].as_array().expect("有供应商");
    let find = |id: &str| {
        providers
            .iter()
            .find(|provider| provider["id"] == id)
            .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
    };
    assert_eq!(find("newapi")["strict_tools"], json!(true));
    assert!(
        find("deepseek").get("strict_tools").is_none(),
        "没标的没有这一格"
    );
}
