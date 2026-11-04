//! 选 Miyu 自己的东西（施工 PC-3，设计 53 第四节；预设是 PC-3 补）：真核心走一遍。`config.schema` 的 `type` 是 `reference`
//! （模型、池都行，同 `models.chat`）、`persona`、`knowledge`、`preset`，控件 `text`；`check` 里模型照引用查（供应商、池没有的
//! `bad_reference`，错误），人格、知识库、预设没有的报警告 `unknown_persona`、`unknown_knowledge`、`unknown_preset`，有的不报
//! （出厂的预设 `full` 算有）；交给脚本、扩展的照原样。

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

[settings.mode]
type = "preset"
name = { en = "Mode" }

[settings.modes]
type = "list"
element = "preset"
name = { en = "Modes" }
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
    home.write("opt/draw/package.toml", DRAW);
    home.write("root/personas/cat/persona.toml", "[voice]\n");
    home.write(
        "etc/config.toml",
        "[draw]\nmodel = \"gone/m\"\npersona = \"cat\"\nkb = \"notes\"\nhelpers = [\"cat\", \"dog\"]\nmode = \"full\"\nmodes = [\"gone\"]\n",
    );
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    for (key, kind) in [
        ("draw.model", "reference"),
        ("draw.persona", "persona"),
        ("draw.kb", "knowledge"),
        ("draw.mode", "preset"),
    ] {
        let one = item(&schema, key);
        assert_eq!(
            (one["type"].clone(), one["control"].clone()),
            (json!(kind), json!("text")),
            "{key}"
        );
    }
    assert_eq!(item(&schema, "draw.helpers")["element"], "persona");
    assert_eq!(item(&schema, "draw.modes")["element"], "preset");
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
            (
                "draw.modes".to_string(),
                "unknown_preset".to_string(),
                "warning".to_string()
            ),
        ],
        "有的人格（cat）、出厂的预设（full）不报：{checked}"
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
    home.write("opt/draw/package.toml", DRAW);
    home.write(
        "etc/config.toml",
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

/// 核心自己的两项也是这两种（施工 PC-4）：终端会话的人格、预设在 `config.schema` 里是 `persona`、`preset`，头照列表让人挑；
/// 写了现在没有的，`check` 报警告，出厂的预设 `full` 不报。
#[tokio::test]
async fn the_terminal_session_picks_a_persona_and_a_preset_too() {
    let home = Home::new();
    home.write(
        "etc/config.toml",
        "[terminal]\npersona = \"ghost\"\npreset = \"full\"\n",
    );
    let mut client = Client::connect(home.core(&miyu_session::testkit::Script::new([])));
    client.hello().await;
    let reply = client
        .call(
            "t1",
            "config.schema",
            json!({"keys": ["terminal.persona", "terminal.preset"]}),
        )
        .await;
    let schema = reply
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(item(&schema, "terminal.persona")["type"], "persona");
    assert_eq!(item(&schema, "terminal.preset")["type"], "preset");
    let checked = client.call("t2", "check", json!({})).await;
    let warned: Vec<(String, String)> = checked["result"]["problems"]
        .as_array()
        .expect("有")
        .iter()
        .filter(|problem| problem["kind"] == "config")
        .map(|problem| {
            (
                problem["key"].as_str().unwrap_or_default().to_string(),
                problem["code"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        warned,
        [(
            "terminal.persona".to_string(),
            "unknown_persona".to_string()
        )],
        "{checked}"
    );
}
