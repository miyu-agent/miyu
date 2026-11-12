//! 包的设置分节（施工 SG-1，`docs/blueprint/packages.md`「配置项」第 1 条、`config.md`「协议」）：真核心走一遍。组里的项带
//! `section`，这个包那一组带 `sections`（名字照连接的语言，`shown_when` 的键写全、值总是列表）；没分组的包两样都不写。
//! 藏着的那一节照旧能写、能读：核心不看 `shown_when`。

use serde_json::{Value, json};

use crate::support::*;

/// 一个照引擎分节的包，前缀 `say`。
const SAY: &str = r#"[package]
protocol = [1, 1]
name = { en = "Say", zh = "说话" }
prefix = "say"

[settings.engine]
type = "option"
choices = ["minimax", "mimo", "openai"]
default = "minimax"
name = { en = "Engine", zh = "引擎" }

[settings.minimax]
name = { en = "MiniMax", zh = "MiniMax 语音" }
shown_when = { engine = "minimax" }

[settings.minimax.voice]
type = "text"
name = { en = "Voice" }

[settings.cloud]
name = { en = "Cloud" }
shown_when = { engine = ["mimo", "openai"] }

[settings.cloud.voice]
type = "text"
name = { en = "Voice" }

[settings.extra.speed]
type = "int"
name = { en = "Speed" }

[ui]
"#;

/// 一个没分组的包。
const CLOCK: &str = r#"[package]
protocol = [1, 1]
name = { en = "Clock", zh = "时钟" }

[settings.port]
type = "int"
name = { en = "Port", zh = "端口" }

[ui]
"#;

async fn schema(home: &Home) -> (Client, Value) {
    let mut client = Client::connect(packaged::core(home));
    client.hello().await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    (client, schema)
}

fn find<'a>(schema: &'a Value, what: &str, id: &str) -> &'a Value {
    let field = if what == "items" { "key" } else { "id" };
    schema[what]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one[field] == id)
        .unwrap_or_else(|| panic!("没有 {id}：{schema}"))
}

#[tokio::test]
async fn grouped_settings_carry_their_section_and_the_group_lists_sections() {
    let home = Home::new();
    home.write("opt/say/package.toml", SAY);
    let (_client, schema) = schema(&home).await;
    assert_eq!(
        find(&schema, "items", "say.minimax.voice")["section"],
        "minimax"
    );
    assert_eq!(
        find(&schema, "items", "say.cloud.voice")["section"],
        "cloud"
    );
    assert_eq!(
        find(&schema, "items", "say.extra.speed")["section"],
        "extra"
    );
    assert!(
        find(&schema, "items", "say.engine")
            .get("section")
            .is_none(),
        "不在组里的不写"
    );
    assert_eq!(
        find(&schema, "groups", "say"),
        &json!({
            "id": "say",
            "name": "说话",
            "page": "packages",
            "sections": [
                {"id": "minimax", "name": "MiniMax 语音",
                 "shown_when": {"key": "say.engine", "is": ["minimax"]}},
                {"id": "cloud", "name": "Cloud",
                 "shown_when": {"key": "say.engine", "is": ["mimo", "openai"]}},
                {"id": "extra", "name": "extra"},
            ],
        }),
        "照连接的语言、没有的照英文、没写的照编号；一个值的也是列表"
    );
}

#[tokio::test]
async fn a_package_without_groups_keeps_its_old_shape() {
    let home = Home::new();
    home.write("opt/clock/package.toml", CLOCK);
    let (_client, schema) = schema(&home).await;
    assert_eq!(
        find(&schema, "groups", "clock"),
        &json!({"id": "clock", "name": "时钟", "page": "packages"})
    );
    let port = find(&schema, "items", "clock.port");
    assert!(port.get("section").is_none(), "{port}");
    assert!(
        schema["groups"]
            .as_array()
            .expect("有")
            .iter()
            .all(|group| group.get("sections").is_none()),
        "没有分组的包，哪一组都不写"
    );
}

#[tokio::test]
async fn a_hidden_section_still_stores_and_reads_its_values() {
    let home = Home::new();
    home.write("opt/say/package.toml", SAY);
    let (mut client, _) = schema(&home).await;
    let set = client
        .call(
            "w1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "say.cloud.voice", "value": "alloy"}]}),
        )
        .await;
    assert!(
        set.get("error").is_none(),
        "引擎是 minimax，cloud 那一节不画，照样能写：{set}"
    );
    let got = client
        .call("g1", "config.get", json!({"keys": ["say.cloud.voice"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["say.cloud.voice"]["value"], "alloy",
        "{got}"
    );
}
