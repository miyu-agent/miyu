//! 包的配置项的词汇（施工 PC-1，设计 53 第一、三节）：真核心走一遍。`config.schema` 里选项带照连接的语言挑的名字、说明（没写名字的
//! 就是值，没写说明的不带）；多行字是 `text` 加 `control: "textarea"`；小数是 `float`、`control: "number"`；滑块是
//! `control: "slider"` 带 `step`。写进系统配置的值照类型收、读得回、交给脚本和扩展的照原样（小数是数，多行照写的）。

use serde_json::{Value, json};

use miyu_endpoint::config::{Config, Environment};
use miyu_session::ConfigSource;

use crate::support::*;

/// 一个只带脚本的生图包：选项带名字、说明，提示词几行，温度是小数滑块，张数是整数滑块，比例是不画滑块的小数。
const DRAW: &str = r#"[package]
name = { en = "Draw", zh = "生图" }

[settings.background]
type = "option"
choices = [
  { value = "auto", name = { zh = "自动", en = "Auto" } },
  { value = "transparent", name = { zh = "透明", en = "Transparent" }, description = { zh = "要配 png、webp", en = "Needs png or webp" } },
  { value = "opaque" },
]
default = "auto"
name = { en = "Background", zh = "背景" }

[settings.prompt]
type = "text"
lines = true
name = { en = "Prompt template" }

[settings.temperature]
type = "number"
min = 0
max = 2
step = 0.1
slider = true
default = 0.7
name = { en = "Temperature" }

[settings.count]
type = "int"
min = 1
max = 8
slider = true
default = 1
name = { en = "Images" }

[settings.ratio]
type = "number"
min = 0
max = 4
name = { en = "Ratio" }
"#;

fn item<'a>(schema: &'a Value, key: &str) -> &'a Value {
    schema["items"]
        .as_array()
        .expect("有项")
        .iter()
        .find(|item| item["key"] == key)
        .unwrap_or_else(|| panic!("没有 {key}：{schema}"))
}

#[tokio::test]
async fn the_schema_says_choice_names_lines_numbers_and_sliders() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    let background = item(&schema, "draw.background");
    assert_eq!(
        background["options"],
        json!([
            {"name": "自动", "value": "auto"},
            {"name": "透明", "value": "transparent", "description": "要配 png、webp"},
            {"name": "opaque", "value": "opaque"},
        ]),
        "照连接的语言（中文）；没写名字的是值，没写说明的不带"
    );
    assert_eq!(background["control"], "select");
    let prompt = item(&schema, "draw.prompt");
    assert_eq!(
        (
            prompt["type"].clone(),
            prompt["control"].clone(),
            prompt["max"].clone()
        ),
        (json!("text"), json!("textarea"), json!(2000))
    );
    let temperature = item(&schema, "draw.temperature");
    assert_eq!(
        (
            temperature["type"].clone(),
            temperature["control"].clone(),
            temperature["step"].clone(),
            temperature["min"].clone(),
            temperature["max"].clone(),
            temperature["default"].clone(),
        ),
        (
            json!("float"),
            json!("slider"),
            json!(0.1),
            json!(0),
            json!(2),
            json!(0.7)
        )
    );
    let count = item(&schema, "draw.count");
    assert_eq!(
        (
            count["type"].clone(),
            count["control"].clone(),
            count["step"].clone()
        ),
        (json!("int"), json!("slider"), json!(1.0))
    );
    let ratio = item(&schema, "draw.ratio");
    assert_eq!(
        (ratio["type"].clone(), ratio["control"].clone()),
        (json!("float"), json!("number"))
    );
    assert!(ratio.get("step").is_none(), "不画滑块的不带：{ratio}");
}

#[tokio::test]
async fn values_are_checked_by_their_type_and_handed_over_as_written() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write(
        "config.toml",
        "[draw]\nbackground = \"transparent\"\nprompt = \"\"\"\nA cat.\nIn the snow.\"\"\"\ntemperature = 1.25\nratio = 2\n",
    );
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let got = client
        .call(
            "g1",
            "config.get",
            json!({"keys": ["draw.background", "draw.prompt", "draw.temperature", "draw.ratio"]}),
        )
        .await;
    let values = &got["result"]["items"];
    assert_eq!(values["draw.background"]["value"], "transparent", "{got}");
    assert_eq!(
        values["draw.prompt"]["value"], "A cat.\nIn the snow.",
        "{got}"
    );
    assert_eq!(values["draw.temperature"]["value"], 1.25, "{got}");
    assert_eq!(values["draw.ratio"]["value"], 2.0, "整数也收：{got}");
    for wrong in [
        "background = \"Auto\"",
        "temperature = 2.5",
        "count = 9",
        "prompt = 3",
    ] {
        home.write("config.toml", &format!("[draw]\n{wrong}\n"));
        let checked = client.call("c1", "check", json!({})).await;
        let problems = checked["result"]["problems"].as_array().expect("有");
        assert!(
            problems
                .iter()
                .any(|problem| problem["file"] == "config.toml" && problem["kind"] == "config"),
            "{wrong}：{checked}"
        );
    }
    // 交给脚本、扩展的照原样（施工 PS-1 那一份）。
    home.write(
        "config.toml",
        "[draw]\nprompt = \"\"\"\nA cat.\nIn the snow.\"\"\"\ntemperature = 1.25\n",
    );
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let items = miyu_endpoint::packages::settle(&mut found, &[]);
    let config = Config::load(&home.root, None, items, Environment::of(&[]));
    assert_eq!(
        config.package_settings("draw").map(Value::Object),
        Some(json!({
            "background": "auto",
            "prompt": "A cat.\nIn the snow.",
            "temperature": 1.25,
            "count": 1,
        }))
    );
}
