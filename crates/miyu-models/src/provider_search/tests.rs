//! 供应商自带的联网搜索（施工 DSR-3）：官方地址、驱动、思考三样都对才交；档案里写坏的读不进来。

use std::collections::BTreeMap;

use miyu_config::Address;
use miyu_drivers::{EFFORT_OFF, EFFORT_ON};
use serde_json::json;

use super::*;
use crate::profile::Profiles;
use crate::provider::{Driver, DriverFrom};

/// 档案里 DeepSeek 写的那一格。
fn deepseek_search() -> ProviderSearch {
    ProviderSearch {
        setting: "deepseek-responses".to_string(),
        driver: "openai-responses".to_string(),
        effort: "high".to_string(),
        replaces: vec![
            ("web_search".to_string(), "deepseek_web_search".to_string()),
            ("web_fetch".to_string(), "deepseek_read_url".to_string()),
        ],
    }
}

/// 照档案 `deepseek` 合出来的一家：地址 `base_url`、驱动 `driver`、档案写没写 `search`。
fn speaking(base_url: Address, driver: Driver, search: Option<ProviderSearch>) -> Provider {
    Provider {
        id: "deepseek".to_string(),
        driver,
        base_url,
        compat: miyu_drivers::openai_chat::Compat::default(),
        key: None,
        images: None,
        catalog: "deepseek".to_string(),
        recognized: None,
        local: false,
        driver_written: true,
        base_url_written: true,
        driver_from: DriverFrom::Config,
        reasoning_written: false,
        headers: BTreeMap::new(),
        placeholders: Vec::new(),
        quota: None,
        search,
    }
}

fn literal(url: &str) -> Address {
    Address::Literal(url.to_string())
}

#[test]
fn only_the_official_address_the_named_driver_and_thinking_on_offer_it() {
    let official = literal("https://api.deepseek.com");
    let search = Some(deepseek_search());
    let on = speaking(official.clone(), Driver::OpenAiResponses, search.clone());
    for effort in [None, Some(EFFORT_ON), Some("high")] {
        assert_eq!(offered(&on, effort), Some(&deepseek_search()), "{effort:?}");
    }
    assert_eq!(offered(&on, Some(EFFORT_OFF)), None, "思考关着的不交");
    // 官方地址写了路径、主机大写的照样算。
    let with_path = speaking(
        literal("HTTPS://API.DEEPSEEK.COM/v1"),
        Driver::OpenAiResponses,
        search.clone(),
    );
    assert!(offered(&with_path, None).is_some());
    for address in [
        literal("https://relay.example.invalid/v1"),
        literal("https://relay.example.invalid/api.deepseek.com"),
        Address::Env("DEEPSEEK_URL".to_string()),
    ] {
        let relay = speaking(address.clone(), Driver::OpenAiResponses, search.clone());
        assert_eq!(offered(&relay, None), None, "{address:?}");
    }
    for driver in [Driver::OpenAiChat, Driver::Anthropic] {
        let other = speaking(official.clone(), driver.clone(), search.clone());
        assert_eq!(offered(&other, None), None, "{driver:?}");
    }
    let without = speaking(official, Driver::OpenAiResponses, None);
    assert_eq!(offered(&without, None), None, "档案没写的不交");
}

/// 官方地址表里没有的一家，档案写了 `search` 也不交：认不出官方地址。
#[test]
fn a_provider_without_a_known_official_address_never_offers_it() {
    let mut other = speaking(
        literal("https://api.deepseek.com"),
        Driver::OpenAiResponses,
        Some(deepseek_search()),
    );
    other.catalog = "openai".to_string();
    assert_eq!(offered(&other, None), None);
}

#[test]
fn the_profile_reads_search_into_pairs() {
    let profiles = Profiles::parse(&json!({"providers": {"deepseek": {"search": {
        "setting": "deepseek-responses",
        "driver": "openai-responses",
        "effort": "high",
        "replaces": [["web_search", "deepseek_web_search"], ["web_fetch", "deepseek_read_url"]]
    }}}}))
    .expect("读得进来");
    assert_eq!(
        profiles.providers["deepseek"].search,
        Some(deepseek_search())
    );
}

#[test]
fn a_badly_written_search_is_not_read() {
    let pairs = |replaces: serde_json::Value| {
        json!({"providers": {"x": {"search": {
            "setting": "s", "driver": "openai-responses", "effort": "high", "replaces": replaces
        }}}})
    };
    for bad in [
        json!({"providers": {"x": {"search": {"setting": "", "driver": "d", "effort": "e", "replaces": []}}}}),
        json!({"providers": {"x": {"search": {"setting": "s", "driver": "", "effort": "e", "replaces": []}}}}),
        json!({"providers": {"x": {"search": {"setting": "s", "driver": "d", "effort": "", "replaces": []}}}}),
        json!({"providers": {"x": {"search": {"setting": "s", "driver": "d", "replaces": []}}}}),
        json!({"providers": {"x": {"search": {"setting": "s", "driver": "d", "effort": "e"}}}}),
        json!({"providers": {"x": {"search": {"setting": "s", "driver": "d", "effort": "e", "replaces": [], "extra": 1}}}}),
        pairs(json!([["", "b"]])),
        pairs(json!([["a", ""]])),
        pairs(json!([["a", "b"], ["a", "c"]])),
        pairs(json!([["a", "b"], ["c", "b"]])),
        pairs(json!([["a", "b", "c"]])),
        pairs(json!([["a"]])),
        pairs(json!(["a"])),
    ] {
        let error = Profiles::parse(&bad).expect_err("读不进来");
        assert!(
            error.starts_with("models/profiles.toml not readable: "),
            "{error}"
        );
    }
    // 同一个名字分在两边的不算重复：换成同名的供应商工具也读得进来。
    assert!(Profiles::parse(&pairs(json!([["a", "a"]]))).is_ok());
}
