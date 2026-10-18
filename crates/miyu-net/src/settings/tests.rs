//! 搜索来源的选项和出厂的来源对得上（施工 N-2 下）：下拉里能选的、`search.json` 里有的，一一对应。

use std::path::Path;

use miyu_config::secret::{Reference, Secret};
use miyu_config::{Value, Values};

use super::{AUTO, ITEMS, Missing, SearchSettings, missing, using};

#[test]
fn the_options_are_auto_and_every_shipped_source() {
    let item = &SearchSettings::ITEMS[0];
    assert_eq!(item.key, "net.search.source");
    let miyu_config::Kind::Option(options) = item.kind else {
        panic!("是选项：{:?}", item.kind);
    };
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/search.json"),
    )
    .expect("读得了");
    let shipped: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
    let mut sources: Vec<&str> = shipped["sources"]
        .as_object()
        .expect("有 sources")
        .keys()
        .map(String::as_str)
        .collect();
    sources.sort_unstable();
    let mut listed: Vec<&str> = options
        .iter()
        .copied()
        .filter(|option| *option != AUTO)
        .collect();
    listed.sort_unstable();
    assert_eq!(listed, sources, "下拉里的来源就是出厂的那几家");
    assert_eq!(options[0], AUTO, "自动排第一，是默认值");
}

/// 「少配了什么」和 `search.json` 里每家的 `needs` 一致（施工 N-3 上）：什么都没配时，要 key 的说要 key，要地址的说要地址，
/// 别的不说。
#[test]
fn the_missing_check_agrees_with_what_each_source_needs() {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/search.json"),
    )
    .expect("读得了");
    let shipped: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
    let values = Values::defaults(&ITEMS.concat());
    for (id, source) in shipped["sources"].as_object().expect("有 sources") {
        let expected = match source["needs"].as_str().expect("写了 needs") {
            "key" => Some(Missing::Key),
            "address" => Some(Missing::Address),
            _ => None,
        };
        assert_eq!(missing(id, &values), expected, "{id}");
    }
}

/// 这一轮的配置：最终值是 `values`，密钥照 `secrets` 取（名字或者环境变量 → 值）。
struct Turn(Values, Vec<(Reference, &'static str)>);

impl miyu_tool::SettingsPort for Turn {
    fn values(&self) -> Values {
        self.0.clone()
    }

    fn secret(&self, reference: &Reference) -> Option<Secret> {
        self.1
            .iter()
            .find(|(name, _)| name == reference)
            .and_then(|(_, value)| Secret::new(value).ok())
    }
}

fn secret(name: &str) -> Value {
    Value::Secret(Reference::Secret(name.to_string()))
}

/// 拼这一次用得上的（施工 N-3 上）：挑的那一家，配了的 key 照引用取，取不到的当没配；SearXNG 的地址可以写成环境变量；
/// DeepSeek 没单独配的照用模型那边 DeepSeek 供应商的 key。
#[test]
fn using_reads_the_picked_source_keys_and_address_from_the_turn() {
    let mut values = Values::defaults(&ITEMS.concat());
    values.set("net.search.source", Value::Text("tavily".into()));
    values.set("net.search.tavily.key", secret("tavily"));
    values.set("net.search.brave.key", secret("nowhere"));
    values.set(
        "net.search.searxng.url",
        Value::Secret(Reference::Env("SEARX".to_string())),
    );
    values.set(
        "providers.ds.base_url",
        Value::Text("https://api.deepseek.com".into()),
    );
    values.set("providers.ds.key", secret("deepseek"));
    let turn = Turn(
        values,
        vec![
            (Reference::Secret("tavily".to_string()), "tvly-1"),
            (Reference::Secret("deepseek".to_string()), "sk-2"),
            (Reference::Env("SEARX".to_string()), "http://searx.local"),
        ],
    );

    let got = using(&turn);

    assert_eq!(got.preferred.as_deref(), Some("tavily"));
    assert_eq!(got.key("tavily"), Some("tvly-1"));
    assert_eq!(got.key("brave"), None, "引用取不到的当没配");
    assert_eq!(
        got.key("deepseek"),
        Some("sk-2"),
        "照用模型那边 DeepSeek 供应商的 key"
    );
    assert_eq!(got.address.as_deref(), Some("http://searx.local"));
    assert!(
        missing("deepseek", &turn.0).is_none(),
        "供应商配了 DeepSeek 的也算配了"
    );
}
