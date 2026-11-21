//! 联网服务的选项和出厂的来源对得上（施工 N-2 下，NF-1 改成选供应商）：下拉里能选的是自动和 `search.json` 里能配 key（施工
//! NS-1）、要 key、要地址的那几家。

use std::path::Path;

use miyu_config::secret::{Reference, Secret};
use miyu_config::{Value, Values};

use super::{AUTO, ITEMS, SearchSettings, using};

/// 「DeepSeek Responses 接口内置」这个选项（设计 72）。
const DEEPSEEK_RESPONSES: &str = "deepseek-responses";

#[test]
fn the_options_are_auto_and_the_sources_that_need_a_key_or_an_address() {
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
        .iter()
        .filter(|(_, source)| {
            matches!(
                source["needs"].as_str(),
                Some("key" | "key_optional" | "address")
            )
        })
        .map(|(id, _)| id.as_str())
        .collect();
    sources.sort_unstable();
    let mut listed: Vec<&str> = options
        .iter()
        .copied()
        .filter(|option| *option != AUTO && *option != DEEPSEEK_RESPONSES)
        .collect();
    listed.sort_unstable();
    assert_eq!(
        listed, sources,
        "下拉里是能配 key、要 key、要地址的几家，只有免费的几家归自动"
    );
    assert_eq!(options[0], AUTO, "自动排第一，是默认值");
}

/// 「DeepSeek Responses 接口内置」（设计 72，施工 DSR-2）在下拉里，接在 DeepSeek 搜索接口后面；它不是 `search.json` 里的一家，
/// 不需要密钥和地址。
#[test]
fn the_deepseek_responses_option_is_listed_and_is_not_a_shipped_source() {
    let miyu_config::Kind::Option(options) = SearchSettings::ITEMS[0].kind else {
        panic!("是选项");
    };
    let at = |name: &str| options.iter().position(|option| *option == name);
    assert_eq!(
        at(DEEPSEEK_RESPONSES),
        at("deepseek").map(|at| at + 1),
        "{options:?}"
    );
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/net/search.json"),
    )
    .expect("读得了");
    let shipped: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
    assert!(shipped["sources"].get(DEEPSEEK_RESPONSES).is_none());
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
}
