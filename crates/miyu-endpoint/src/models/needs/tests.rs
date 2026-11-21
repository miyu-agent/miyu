//! 一家供应商真要哪几项（施工 FX-11）：扩展驱动照清单 `[[driver]]` 的 `key`、`base_url`，没写的是可有可无；内置驱动要密钥、要地址，
//! 本机的服务密钥可有可无、目录认出来的地址可有可无；扩展没装、驱动名对不上的不写。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_config::Values;
use miyu_store::packages::{Found, Layer};

use super::needs;

/// 装着的 `gravity` 包：两个驱动，`antigravity` 不要地址、不要密钥，`plain` 什么都没写。
fn packages() -> Vec<Found> {
    let text = "[package]\nprotocol = [1, 1]\nname = { en = \"Gravity\" }\n\n[command]\nname = \"gravity\"\nprogram = \"gravity\"\nabout = { en = \"Gravity\" }\n\n[process]\ncapabilities = [\"model.driver\"]\n\n[[driver]]\nname = \"antigravity\"\nkey = \"none\"\nbase_url = \"none\"\n\n[[driver]]\nname = \"plain\"\n";
    vec![Found {
        id: "gravity".to_string(),
        layer: Layer::Home,
        path: PathBuf::from("gravity/package.toml"),
        read: Ok(miyu_config::package::read(text).expect("合写法")),
    }]
}

/// 什么都没写的配置。
fn values() -> Values {
    Values::defaults(miyu_models::settings::ProviderSettings::ITEMS)
}

fn of(entry: &Value, values: &Values) -> Option<Value> {
    needs(
        entry,
        values,
        entry["id"].as_str().expect("有编号"),
        &packages(),
    )
}

#[test]
fn an_extension_driver_says_what_its_manifest_declares() {
    let values = values();
    assert_eq!(
        of(
            &json!({"id": "agy", "driver": "gravity/antigravity"}),
            &values
        ),
        Some(json!({"key": "none", "base_url": "none"})),
        "Antigravity 这种：没有地址、不要密钥"
    );
    assert_eq!(
        of(&json!({"id": "p", "driver": "gravity/plain"}), &values),
        Some(json!({"key": "optional", "base_url": "optional"})),
        "没写的可有可无"
    );
    assert_eq!(
        of(&json!({"id": "x", "driver": "gravity/missing"}), &values),
        None
    );
    assert_eq!(
        of(&json!({"id": "y", "driver": "absent/antigravity"}), &values),
        None
    );
}

#[test]
fn a_built_in_driver_needs_a_key_and_an_address_unless_it_can_do_without() {
    let values = values();
    assert_eq!(
        of(
            &json!({"id": "relay", "driver": "openai-chat", "base_url": "https://relay.invalid/v1"}),
            &values
        ),
        Some(json!({"key": "required", "base_url": "required"}))
    );
    assert_eq!(
        of(
            &json!({"id": "deepseek", "driver": "openai-chat", "base_url": "https://api.deepseek.com", "catalog": {"provider": "deepseek"}}),
            &values
        ),
        Some(json!({"key": "required", "base_url": "optional"})),
        "目录认出来的地址照目录推"
    );
    assert_eq!(
        of(
            &json!({"id": "ollama", "driver": "openai-chat", "base_url": "http://127.0.0.1:11434/v1"}),
            &values
        ),
        Some(json!({"key": "optional", "base_url": "required"})),
        "本机的服务密钥可有可无"
    );
}
