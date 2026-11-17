//! 「已继承」的来源（施工 NS-1）：联网包的 DeepSeek 的 key 没写、有官方的 DeepSeek 供应商的写 `inherited`、指那一家的 key；
//! 自己写了的、中转站的、别的项照原来的写法。

use serde_json::json;

use miyu_config::secret::Reference;
use miyu_config::{Layer, Value, Values};

use super::*;

const KEY: &str = "net.search.deepseek.key";

/// 照 `(键, 值)` 写的一份最终值。
fn values(pairs: &[(&str, &str)]) -> Values {
    let mut values = Values::default();
    for (key, value) in pairs {
        let value = match key.ends_with(".key") {
            true => Value::Secret(Reference::Secret((*value).to_string())),
            false => Value::Text((*value).to_string().into()),
        };
        values.set(key, value);
    }
    values
}

#[test]
fn an_unset_key_inherits_the_official_provider() {
    let official = values(&[("providers.deepseek.key", "ds")]);
    assert_eq!(
        origin(KEY, None, &official),
        Some(json!({"layer": "inherited", "from": "providers.deepseek.key"}))
    );
    // 自己写了的照写的地方。
    let written = Origin::File {
        layer: Layer::System,
        line: 3,
    };
    assert_eq!(origin(KEY, Some(&written), &official), None);
    // 只在登记了的项上。
    assert_eq!(origin("net.search.exa.key", None, &official), None);
}

#[test]
fn a_relay_or_no_provider_is_not_inherited() {
    let relay = values(&[
        ("providers.relay.catalog", "deepseek"),
        (
            "providers.relay.base_url",
            "https://relay.example.invalid/v1",
        ),
        ("providers.relay.key", "relay"),
    ]);
    assert_eq!(
        origin(KEY, Some(&Origin::Default), &relay),
        None,
        "中转站的不借"
    );
    assert_eq!(origin(KEY, None, &Values::default()), None);
}
