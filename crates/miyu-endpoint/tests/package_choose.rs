//! 选 Miyu 自己的东西（施工 PC-3，设计 53 第四节）：真核心走一遍。`config.schema` 的 `type` 是 `reference`（模型、池都行，同
//! `models.chat`）、`persona`、`knowledge`，控件 `text`；`check` 里模型照引用查（供应商、池没有的 `bad_reference`，错误），人格、
//! 知识库没有的报警告 `unknown_persona`、`unknown_knowledge`，有的不报；交给脚本、扩展的照原样。

use serde_json::{Value, json};

use miyu_endpoint::config::{Config, Environment};
use miyu_session::ConfigSource;

use crate::support::*;

const DRAW: &str = r#"[package]
name = { en = "Draw" }

[settings.model]
type = "model"
name = { en = "Model" }

[settings.persona]
type = "persona"
name = { en = "Persona" }

[settings.kb]
type = "knowledge"
name = { en = "KB" }

[settings.helpers]
type = "list"
element = "persona"
name = { en = "Helpers" }
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
async fn the_schema_says_what_to_choose_and_check_warns_about_missing_ones() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write("personas/cat/persona.toml", "[voice]\n");
    home.write(
        "config.toml",
        "[draw]\nmodel = \"gone/m\"\npersona = \"cat\"\nkb = \"notes\"\nhelpers = [\"cat\", \"dog\"]\n",
    );
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    for (key, kind) in [
        ("draw.model", "reference"),
        ("draw.persona", "persona"),
        ("draw.kb", "knowledge"),
    ] {
        let one = item(&schema, key);
        assert_eq!(
            (one["type"].clone(), one["control"].clone()),
            (json!(kind), json!("text")),
            "{key}"
        );
    }
    assert_eq!(item(&schema, "draw.helpers")["element"], "persona");
    let created = client.call("k0", "kb.create", json!({"kb": "other"})).await;
    assert!(created.get("error").is_none(), "{created}");
    let checked = client.call("c1", "check", json!({})).await;
    let mut found: Vec<(String, String, String)> = checked["result"]["problems"]
        .as_array()
        .expect("有")
        .iter()
        .filter(|problem| problem["kind"] == "config")
        .map(|problem| {
            (
                problem["key"].as_str().unwrap_or_default().to_string(),
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["level"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            (
                "draw.helpers".to_string(),
                "unknown_persona".to_string(),
                "warning".to_string()
            ),
            (
                "draw.kb".to_string(),
                "unknown_knowledge".to_string(),
                "warning".to_string()
            ),
            (
                "draw.model".to_string(),
                "bad_reference".to_string(),
                "error".to_string()
            ),
        ],
        "有的人格（cat）不报：{checked}"
    );
    // 建了这个知识库，就不报了。
    let created = client.call("k1", "kb.create", json!({"kb": "notes"})).await;
    assert!(created.get("error").is_none(), "{created}");
    let checked = client.call("c2", "check", json!({})).await;
    assert!(
        !checked.to_string().contains("unknown_knowledge"),
        "{checked}"
    );
}

#[test]
fn choices_are_handed_over_as_written() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write(
        "config.toml",
        "[draw]\nmodel = \"@fast\"\npersona = \"cat\"\nhelpers = [\"cat\"]\n",
    );
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let items = miyu_endpoint::packages::settle(&mut found, &[]);
    let config = Config::load(&home.root, None, items, Environment::of(&[]));
    assert_eq!(
        config.package_settings("draw").map(Value::Object),
        Some(json!({"model": "@fast", "persona": "cat", "helpers": ["cat"]})),
        "照原样交，核心不替它解析"
    );
}
