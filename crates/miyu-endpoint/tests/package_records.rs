//! 列表里每一项是一组字段（施工 PC-2，设计 53 第二节）：真核心走一遍。`config.schema` 带 `element: "record"`、`title`、`fields`
//! （每个字段格同普通项）；`config.set` 整条换，写成 `[[<键>]]`，`config.get` 读回对象的数组、密钥照旧是引用；密钥字段写明文的
//! 整条拒、配置文件和密钥库都不动；写错的报到「第几项的哪个字段」；交给脚本、扩展的是对象的数组，密钥是真值，没写的字段照默认值。

use serde_json::{Value, json};

use miyu_endpoint::config::{Config, Environment};
use miyu_session::ConfigSource;

use crate::support::*;

/// 生图包：一组账号，备注当标题，地址有默认值，密钥，模式是带名字的选项、隐藏。
const DRAW: &str = r#"[package]
name = { en = "Draw", zh = "生图" }

[settings.accounts]
type = "list"
element = "record"
title = "name"
applies = "now"
name = { en = "Accounts", zh = "账号" }

[settings.accounts.fields.name]
type = "text"
name = { zh = "备注", en = "Name" }

[settings.accounts.fields.base_url]
type = "url"
default = "https://api.example.com/v1"
name = { zh = "接口地址" }

[settings.accounts.fields.key]
type = "secret"
name = { zh = "Key" }

[settings.accounts.fields.mode]
type = "option"
choices = [{ value = "fast", name = { zh = "快" } }, { value = "good" }]
default = "fast"
hidden = true
name = { zh = "模式" }
"#;

fn item<'a>(schema: &'a Value, key: &str) -> &'a Value {
    schema["items"]
        .as_array()
        .expect("有项")
        .iter()
        .find(|item| item["key"] == key)
        .unwrap_or_else(|| panic!("没有 {key}：{schema}"))
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(packaged::core(home));
    client.hello().await;
    client
}

#[tokio::test]
async fn the_schema_lists_the_fields_of_a_record_list() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    let mut client = connected(&home).await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    let accounts = item(&schema, "draw.accounts");
    assert_eq!(
        (
            accounts["type"].clone(),
            accounts["element"].clone(),
            accounts["title"].clone(),
            accounts["control"].clone(),
            accounts["name"].clone(),
        ),
        (
            json!("list"),
            json!("record"),
            json!("name"),
            json!("list"),
            json!("账号")
        )
    );
    let fields = accounts["fields"].as_array().expect("有字段");
    let keys: Vec<&str> = fields
        .iter()
        .filter_map(|field| field["key"].as_str())
        .collect();
    assert_eq!(keys, ["name", "base_url", "key", "mode"], "照清单的先后");
    assert_eq!(
        (
            fields[0]["type"].clone(),
            fields[0]["name"].clone(),
            fields[0]["control"].clone()
        ),
        (json!("text"), json!("备注"), json!("text")),
        "字段的名字照连接的语言"
    );
    assert_eq!(fields[1]["default"], "https://api.example.com/v1");
    assert_eq!(fields[2]["type"], "secret");
    assert_eq!(
        fields[3]["options"],
        json!([{"name": "快", "value": "fast"}, {"name": "good", "value": "good"}])
    );
    assert_eq!(fields[3]["hidden"], true);
}

#[tokio::test]
async fn a_record_list_is_set_whole_and_read_back_with_references() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write(
        "config.toml",
        "# 我的配置\n[draw]\n\n[ui]\nlanguage = \"zh\"\n",
    );
    let mut client = connected(&home).await;
    let accounts = json!([
        {"name": "Home", "key": {"secret": "home"}},
        {"name": "Work", "base_url": "https://work.example.com/v1", "key": {"env": "WORK_KEY"}, "mode": "good"},
    ]);
    let set = client
        .call(
            "c1",
            "config.set",
            json!({"changes": [{"key": "draw.accounts", "value": accounts}]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let written = std::fs::read_to_string(home.root.path().join("config.toml")).expect("写了");
    assert!(
        written.starts_with("# 我的配置\n[draw]\n"),
        "别的字节照原样：{written}"
    );
    assert_eq!(written.matches("[[draw.accounts]]").count(), 2, "{written}");
    assert!(
        written.find("[[draw.accounts]]") < written.find("[ui]"),
        "跟在 [draw] 后面、[ui] 前面：{written}"
    );
    let got = client
        .call("g1", "config.get", json!({"keys": ["draw.accounts"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["draw.accounts"]["value"], accounts,
        "读回对象的数组，密钥照旧是引用：{got}"
    );
    // 换成空的。
    let cleared = client
        .call(
            "c2",
            "config.set",
            json!({"changes": [{"key": "draw.accounts", "value": []}]}),
        )
        .await;
    assert!(cleared.get("error").is_none(), "{cleared}");
    let written = std::fs::read_to_string(home.root.path().join("config.toml")).expect("写了");
    assert!(
        written.contains("[draw]\naccounts = []\n") && !written.contains("[[draw.accounts]]"),
        "{written}"
    );
}

#[tokio::test]
async fn wrong_records_are_refused_at_their_field_and_plain_keys_never_land() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write("config.toml", "[draw]\n");
    let mut client = connected(&home).await;
    let before = std::fs::read(home.root.path().join("config.toml")).expect("在");
    let refused = |reply: &Value| -> Vec<(String, String)> {
        assert_eq!(
            reply["error"]["data"]["reason"], "config_invalid",
            "{reply}"
        );
        reply["error"]["data"]["problems"]
            .as_array()
            .expect("有问题")
            .iter()
            .map(|problem| {
                (
                    problem["key"].as_str().unwrap_or_default().to_string(),
                    problem["code"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    };
    let plain = client
        .call(
            "c1",
            "config.set",
            json!({"changes": [{"key": "draw.accounts", "value": [
                {"name": "Home", "key": {"secret": "home"}},
                {"name": "Leak", "key": "sk-plain-text"},
            ]}]}),
        )
        .await;
    assert_eq!(
        refused(&plain),
        [("draw.accounts[1].key".to_string(), "wrong_type".to_string())],
        "密钥字段只收引用，明文整条拒"
    );
    let written = std::fs::read(home.root.path().join("config.toml")).expect("在");
    assert_eq!(written, before, "配置文件一个字节都不动");
    let secrets =
        std::fs::read_to_string(home.root.path().join("secrets.toml")).unwrap_or_default();
    assert!(!secrets.contains("sk-plain-text"), "明文不落进密钥库");
    assert!(
        !String::from_utf8_lossy(&written).contains("sk-plain-text"),
        "明文不落进配置文件"
    );
    let wrong = client
        .call(
            "c2",
            "config.set",
            json!({"changes": [{"key": "draw.accounts", "value": [
                {"base_url": "not a url", "key": {"secret": "k"}, "note": 1},
            ]}]}),
        )
        .await;
    assert_eq!(
        refused(&wrong),
        [
            (
                "draw.accounts[0].base_url".to_string(),
                "bad_format".to_string()
            ),
            (
                "draw.accounts[0].note".to_string(),
                "unknown_key".to_string()
            ),
            (
                "draw.accounts[0].name".to_string(),
                "missing_field".to_string()
            ),
        ]
    );
    let many: Vec<Value> = (0..101)
        .map(|n| json!({"name": format!("A{n}"), "key": {"secret": "k"}}))
        .collect();
    let too_many = client
        .call(
            "c3",
            "config.set",
            json!({"changes": [{"key": "draw.accounts", "value": many}]}),
        )
        .await;
    assert_eq!(
        refused(&too_many),
        [("draw.accounts".to_string(), "out_of_range".to_string())]
    );
    // 一项单改不收：字段不是能改的键。
    let one = client
        .call(
            "c4",
            "config.set",
            json!({"changes": [{"key": "draw.accounts[0].name", "value": "X"}]}),
        )
        .await;
    assert!(one.get("error").is_some(), "{one}");
    // 手改写错的 check 报到字段。
    home.write(
        "config.toml",
        "[[draw.accounts]]\nname = \"A\"\nkey = \"sk-plain\"\n",
    );
    let checked = client.call("k1", "check", json!({})).await;
    let problems = checked["result"]["problems"].as_array().expect("有");
    assert!(
        problems
            .iter()
            .any(|problem| problem["key"] == "draw.accounts[0].key"
                && problem["code"] == "wrong_type"),
        "{checked}"
    );
}

#[test]
fn records_are_handed_over_as_objects_with_real_keys_and_defaults() {
    let home = Home::new();
    home.write("packages/draw/package.toml", DRAW);
    home.write(
        "config.toml",
        "[[draw.accounts]]\nname = \"Home\"\nkey = { secret = \"home\" }\n\n[[draw.accounts]]\nname = \"Gone\"\nkey = { secret = \"nothing\" }\nmode = \"good\"\n",
    );
    home.write("secrets.toml", "home = \"sk-home\"\n");
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root);
    let items = miyu_endpoint::packages::settle(&mut found, &[]);
    let config = Config::load(&home.root, None, items, Environment::of(&[]));
    assert_eq!(
        config.package_settings("draw").map(Value::Object),
        Some(json!({"accounts": [
            {"name": "Home", "base_url": "https://api.example.com/v1", "key": "sk-home", "mode": "fast"},
            {"name": "Gone", "base_url": "https://api.example.com/v1", "mode": "good"},
        ]})),
        "密钥是真值，取不到的那个字段不放；没写的字段照默认值"
    );
}
