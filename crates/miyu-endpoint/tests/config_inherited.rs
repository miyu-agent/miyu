//! 「已继承」的来源和英文名（施工 NS-1，项目主人 2026-11-17 定；网页 W-1206 要的英文名）：联网包的 DeepSeek 的 key 没写、模型
//! 那边有官方的 DeepSeek 供应商的，`config.get` 的来源是 `{"layer":"inherited","from":"providers.<编号>.key"}`、值照旧是
//! `null`；中转站的、自己写了的照原来的来源。中文连接的 `config.schema` 每一项、每一组多一个 `name_en`，英文连接的不给。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

miyu_config::settings! {
    /// 照联网包的样子声明的那一项（核心的清单在 `miyu-core`，这里只要这一项）。
    pub struct NetKey in "net.search.deepseek" {
        /// `{ secret = … }` 或 `{ env = … }`。
        key: Option<miyu_config::secret::Reference> = none {
            kind: secret,
            layers: [System],
            applies: next_turn,
            ui: { page: "packages", group: "net_keys", control: text },
        },
    }
}

/// 照 `system` 这份配置起核心、握手（系统语言 `locale`），问 `config.get` 那一项，交回它。
async fn got(system: &str, locale: &str) -> (Value, Client) {
    let home = Home::new();
    home.write("etc/config.toml", system);
    let extra = [
        miyu_models::settings::ProviderSettings::ITEMS,
        NetKey::ITEMS,
    ]
    .concat();
    let core = home.core_with_items(&Script::new([]), None, &[], &extra);
    let mut client = Client::connect(core);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"}, "token": TOKEN, "locale": locale});
    client.call("hello-1", "hello", hello).await;
    let keys = json!({"keys": ["net.search.deepseek.key"]});
    let reply = client.call("c1", "config.get", keys).await;
    (
        reply["result"]["items"]["net.search.deepseek.key"].clone(),
        client,
    )
}

#[tokio::test]
async fn an_unset_key_shows_where_it_is_inherited_from() {
    let official = "[providers.deepseek]\nkey = { secret = \"ds\" }\n";
    let (item, _) = got(official, "zh-CN").await;
    assert_eq!(
        item,
        json!({"origin": {"layer": "inherited", "from": "providers.deepseek.key"}, "value": null})
    );
    let relay = "[providers.relay]\ncatalog = \"deepseek\"\nbase_url = \"https://relay.example.invalid/v1\"\nkey = { secret = \"r\" }\n";
    let (item, _) = got(relay, "zh-CN").await;
    assert_eq!(item, Value::Null, "中转站的不借：哪儿都没写的照旧不列");
    let own = format!("{official}\n[net.search.deepseek]\nkey = {{ secret = \"mine\" }}\n");
    let (item, _) = got(&own, "zh-CN").await;
    assert_eq!(item["origin"]["layer"], json!("system"), "自己写了的照写的");
}

#[tokio::test]
async fn a_chinese_schema_also_names_items_and_groups_in_english() {
    let (_, mut client) = got("", "zh-CN").await;
    let reply = client
        .call("c2", "config.schema", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(reply["result"]["items"][0]["name"], json!("界面语言"));
    assert_eq!(
        reply["result"]["items"][0]["name_en"],
        json!("Interface language")
    );
    assert_eq!(reply["result"]["groups"][0]["name_en"], json!("Display"));
    let (_, mut client) = got("", "en-US").await;
    let reply = client
        .call("c2", "config.schema", json!({"keys": ["ui.language"]}))
        .await;
    assert!(
        reply["result"]["items"][0].get("name_en").is_none(),
        "{reply}"
    );
    assert!(reply["result"]["groups"][0].get("name_en").is_none());
}
