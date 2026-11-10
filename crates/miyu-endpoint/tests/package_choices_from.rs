//! 选项由包在运行时给、语音平台主人不用填号（施工 SG-2，`docs/blueprint/packages.md`「配置项」第 1 条、`config.md`「协议」）：
//! 真核心走一遍。清单写了 `choices_from` 的项，`config.schema` 里带 `{"package", "method"}`，没写的项不带；值照旧是普通
//! 文字，不在列表里的照样写得进。平台是 `voice` 的包，`package.list` 的 `connection` 带 `owner_builtin`，别的平台不带。

use serde_json::{Value, json};

use crate::support::*;

/// 一个选麦克风的包，前缀 `ear`：一项写了 `choices_from`（在组里），一项没写。
const EAR: &str = r#"[package]
protocol = [1, 1]
name = { en = "Ear", zh = "耳朵" }
prefix = "ear"

[settings.input.mic]
type = "text"
name = { en = "Microphone", zh = "麦克风" }
choices_from = "devices.list"

[settings.wake]
type = "text"
name = { en = "Wake word", zh = "唤醒词" }

[ui]
"#;

/// 一个平台桥 `id`：接的平台是 `platform`（照清单的写法，带引号或者是数组）。
fn bridge(id: &str, platform: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"miyu-{id}\"\nabout = {{ en = \"Bridge\" }}\n\n[connection]\nplatform = {platform}\n\n[process]\n"
    )
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(packaged::core(home));
    client.hello().await;
    client
}

fn item<'a>(schema: &'a Value, key: &str) -> &'a Value {
    schema["items"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["key"] == key)
        .unwrap_or_else(|| panic!("没有 {key}：{schema}"))
}

#[tokio::test]
async fn a_setting_says_which_method_gives_its_choices() {
    let home = Home::new();
    home.write("opt/miyu-ear/package.toml", EAR);
    let mut client = connected(&home).await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    let mic = item(&schema, "ear.input.mic");
    assert_eq!(
        mic["choices_from"],
        json!({"package": "miyu-ear", "method": "devices.list"}),
        "包的编号（不是前缀）加清单写的方法名：{mic}"
    );
    assert_eq!(mic["type"], "text", "照旧是文字：{mic}");
    assert!(
        item(&schema, "ear.wake").get("choices_from").is_none(),
        "没写的不带"
    );
    let carrying: Vec<&Value> = schema["items"]
        .as_array()
        .expect("有")
        .iter()
        .filter(|one| one.get("choices_from").is_some())
        .collect();
    assert_eq!(
        carrying.len(),
        1,
        "别的项（核心自己的）都不带：{carrying:?}"
    );
}

#[tokio::test]
async fn a_value_outside_the_choices_is_still_stored() {
    let home = Home::new();
    home.write("opt/miyu-ear/package.toml", EAR);
    let mut client = connected(&home).await;
    let set = client
        .call(
            "w1",
            "config.set",
            json!({"layer": "system", "changes": [{"key": "ear.input.mic", "value": "拔掉了的那一个"}]}),
        )
        .await;
    assert!(
        set.get("error").is_none(),
        "核心不照列表查，没人登记那个方法也写得进：{set}"
    );
    let got = client
        .call("g1", "config.get", json!({"keys": ["ear.input.mic"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["ear.input.mic"]["value"], "拔掉了的那一个",
        "{got}"
    );
}

#[tokio::test]
async fn the_voice_platform_says_its_owner_is_built_in() {
    let home = Home::new();
    let mine = |id: &str, platform: &str| {
        home.write(&format!("opt/{id}/package.toml"), &bridge(id, platform));
    };
    mine("ears", "\"voice\"");
    mine("qbridge", "\"qq\"");
    mine("both", "[\"voice\", \"tg\"]");
    let mut client = connected(&home).await;
    let listed = client.call("l1", "package.list", json!({})).await;
    let connection = |id: &str| {
        listed["result"]["packages"]
            .as_array()
            .and_then(|items| items.iter().find(|one| one["package"] == id))
            .unwrap_or_else(|| panic!("没有 {id}：{listed}"))["connection"]
            .clone()
    };
    assert_eq!(
        connection("ears"),
        json!({"platform": "voice", "owner_builtin": true}),
        "语音的主人不用在对应表里填号"
    );
    assert_eq!(
        connection("qbridge"),
        json!({"platform": "qq"}),
        "别的平台不带这一格，和原来一样"
    );
    assert_eq!(
        connection("both"),
        json!({"platform": "voice", "platforms": ["voice", "tg"], "owner_builtin": true}),
        "照 platform（第一个平台）算"
    );
}
