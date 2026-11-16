//! DeepSeek 官方的余额（施工 QU-2）：档案的写法、只在供应商的地址是官方的时候查（改了查的地址也一样）、读官方的回应。

use miyu_config::secret::Reference;
use miyu_config::{Value as ConfigValue, Values};
use serde_json::json;

use super::super::*;
use crate::profile::Profiles;

/// 官方文档的样子，多一格没见过的。
const REPLY: &str = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"2.00","topped_up_balance":"10.34"},{"currency":"USD","total_balance":"0.50","granted_balance":"0.00","topped_up_balance":"0.50","later":1}],"more":"x"}"#;

fn deepseek() -> Quota {
    Quota {
        url: "https://api.deepseek.com/user/balance".to_string(),
        format: QuotaFormat::DeepseekBalance,
    }
}

fn text(text: &str) -> ConfigValue {
    ConfigValue::Text(text.to_string().into())
}

/// 供应商 `deepseek` 照 `pairs` 写（键省掉 `providers.deepseek.`）。
fn values(pairs: &[(&str, ConfigValue)]) -> Values {
    let mut values = Values::default();
    for (key, value) in pairs {
        values.set(&format!("providers.deepseek.{key}"), value.clone());
    }
    values
}

#[test]
fn the_profile_takes_the_deepseek_balance_format() {
    let profiles = Profiles::parse(&json!({"providers": {"deepseek": {
        "quota": {"url": "https://api.deepseek.com/user/balance", "format": "deepseek-balance"}
    }}}))
    .expect("读得进来");
    assert_eq!(profiles.providers["deepseek"].quota, Some(deepseek()));
}

#[test]
fn the_balance_is_asked_only_where_the_provider_talks_to_the_official_address() {
    let at = |values: &Values| settled(Some(&deepseek()), values, "deepseek", "deepseek");
    for official in [
        values(&[]),
        values(&[("base_url", text("https://api.deepseek.com"))]),
        values(&[("base_url", text("https://API.deepseek.com/v1"))]),
    ] {
        assert_eq!(at(&official), Some(deepseek()), "{official:?}");
    }
    for elsewhere in [
        values(&[("base_url", text("https://relay.example.invalid/v1"))]),
        values(&[(
            "base_url",
            text("https://relay.example.invalid/api.deepseek.com"),
        )]),
        values(&[(
            "base_url",
            ConfigValue::Secret(Reference::Env("DS_URL".to_string())),
        )]),
    ] {
        assert_eq!(at(&elsewhere), None, "地址不是官方的不查：{elsewhere:?}");
    }
    let moved = |base: &str| {
        let mut values = values(&[
            ("base_url", text(base)),
            ("quota.url", text("http://127.0.0.1:9/balance")),
        ]);
        values.set(
            "providers.deepseek.key",
            ConfigValue::Secret(Reference::Env("DS_KEY".to_string())),
        );
        at(&values).map(|quota| quota.url)
    };
    assert_eq!(
        moved("https://api.deepseek.com"),
        Some("http://127.0.0.1:9/balance".to_string()),
        "官方地址的照配置改查的地址"
    );
    assert_eq!(
        moved("https://relay.example.invalid/v1"),
        None,
        "改了查的地址也只在供应商的地址是官方的时候查"
    );
    let relay = values(&[("base_url", text("https://relay.example.invalid/v1"))]);
    assert!(
        settled(Some(&deepseek()), &relay, "deepseek", "opencode-go").is_some(),
        "认不出官方地址的一家不管地址（QU-1 照旧）"
    );
}

#[test]
fn the_reply_reads_into_a_balance_with_the_amounts_as_written() {
    let reading = read(QuotaFormat::DeepseekBalance, REPLY.as_bytes()).expect("读得出");
    assert!(reading.windows.is_empty());
    let balance = reading.balance.expect("有余额");
    assert!(balance.available);
    assert_eq!(
        balance.items,
        [
            BalanceItem {
                currency: "CNY".to_string(),
                total: "12.34".to_string(),
                granted: "2.00".to_string(),
                topped_up: "10.34".to_string(),
            },
            BalanceItem {
                currency: "USD".to_string(),
                total: "0.50".to_string(),
                granted: "0.00".to_string(),
                topped_up: "0.50".to_string(),
            },
        ]
    );
    let empty = read(
        QuotaFormat::DeepseekBalance,
        br#"{"is_available":false,"balance_infos":[]}"#,
    )
    .expect("一种币都没有的照样交");
    assert_eq!(
        empty.balance,
        Some(Balance {
            available: false,
            items: Vec::new()
        })
    );
}

#[test]
fn a_broken_balance_spoils_the_reply() {
    for broken in [
        "not json",
        "{}",
        r#"{"balance_infos":[]}"#,
        r#"{"is_available":"true","balance_infos":[]}"#,
        r#"{"is_available":true}"#,
        r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":12.34,"granted_balance":"0","topped_up_balance":"12.34"}]}"#,
        r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"1","granted_balance":"0"}]}"#,
        r#"{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"}}}"#,
    ] {
        assert!(
            read(QuotaFormat::DeepseekBalance, broken.as_bytes()).is_err(),
            "{broken}"
        );
    }
}
