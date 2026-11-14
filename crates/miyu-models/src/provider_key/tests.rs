//! 认主人配的哪一家是目录里的某一家（施工 PK-1）：写了 `catalog` 的照它，没写的看地址，都没写的看编号；别家的 key 不交。

use miyu_config::secret::Reference;
use miyu_config::{Value, Values};

use super::serving;

fn secret(name: &str) -> Value {
    Value::Secret(Reference::Secret(name.to_string()))
}

/// 照 `(键, 值)` 写的一份最终值。
fn values(pairs: &[(&str, Value)]) -> Values {
    let mut values = Values::default();
    for (key, value) in pairs {
        values.set(key, value.clone());
    }
    values
}

fn text(text: &str) -> Value {
    Value::Text(text.to_string().into())
}

#[test]
fn a_provider_counts_by_catalog_then_official_address_then_id() {
    let by_catalog = values(&[
        ("providers.relay.catalog", text("deepseek")),
        (
            "providers.relay.base_url",
            text("https://relay.example.invalid/v1"),
        ),
        ("providers.relay.key", secret("relay")),
    ]);
    assert_eq!(
        serving(&by_catalog, "deepseek"),
        Some(("relay".to_string(), Reference::Secret("relay".to_string()))),
        "写了 catalog 的照它"
    );
    let by_address = values(&[
        ("providers.ds.base_url", text("https://api.deepseek.com/v1")),
        ("providers.ds.key", secret("ds")),
    ]);
    assert_eq!(
        serving(&by_address, "deepseek").map(|(id, _)| id),
        Some("ds".to_string()),
        "地址里有官方地址"
    );
    let by_id = values(&[("providers.deepseek.key", secret("deepseek"))]);
    assert_eq!(
        serving(&by_id, "deepseek").map(|(id, _)| id),
        Some("deepseek".to_string()),
        "第一次接入写的样子：只有编号"
    );
}

#[test]
fn other_providers_never_count() {
    let other = values(&[
        ("providers.or.catalog", text("openrouter")),
        ("providers.or.base_url", text("https://api.deepseek.com")),
        ("providers.or.key", secret("or")),
        (
            "providers.deepseek.base_url",
            text("https://relay.example.invalid"),
        ),
        ("providers.deepseek.key", secret("relay")),
        (
            "providers.env.base_url",
            Value::Secret(Reference::Env("DS_URL".to_string())),
        ),
        ("providers.env.key", secret("env")),
        ("providers.nokey.catalog", text("deepseek")),
    ]);
    assert_eq!(
        serving(&other, "deepseek"),
        None,
        "写了别家的 catalog、地址不是官方的、地址是环境变量、没写 key 的都不算"
    );
    let two = values(&[
        ("providers.b.catalog", text("deepseek")),
        ("providers.b.key", secret("b")),
        ("providers.a.catalog", text("deepseek")),
        ("providers.a.key", secret("a")),
    ]);
    assert_eq!(
        serving(&two, "deepseek").map(|(id, _)| id),
        Some("a".to_string()),
        "几家都对上的照编号排第一的"
    );
    assert_eq!(serving(&two, "openrouter"), None, "没配那一家的没有");
}
