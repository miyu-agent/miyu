//! 联网这个包的配置项（施工 N-2 下，设计 36 第三节第 1 条，项目主人 2026-10-18 定）：设置页一个「搜索来源」下拉。
//!
//! `auto` 照出厂的顺序（`search.json` 的 `order`）；选了一家的一直先用它，它用不了（出错、限额冷却着）才照出厂的顺序兜底
//! （第二节第 3 条）。下一轮生效：会话 actor 回合开始冻结配置，`web_search` 照调用带来的那一份读（`miyu_tool::SettingsPort`）。
//! 核心替联网这个内置包声明，包没装的设置页不画（`miyu-core` 的 `settings::Packaged`）。

use miyu_config::Address;
use miyu_config::secret::Reference;

miyu_config::settings! {
    /// 搜索的配置。
    pub struct SearchSettings in "net.search" {
        /// 搜索来源：`auto` 照出厂的顺序，或者先用定的那一家。
        source: String = "auto" {
            kind: option [
                "auto", "exa", "parallel", "firecrawl", "anysearch", "you", "deepseek", "tavily", "brave",
                "searxng", "duckduckgo", "bing"
            ],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: select },
        },
    }
}

miyu_config::settings! {
    /// Exa 自己的 key（施工 N-3 上）：配了的走自己的额度，不配照免费额度。
    pub struct ExaKey in "net.search.exa" {
        /// `{ secret = … }` 或 `{ env = … }`。
        key: Option<Reference> = none {
            kind: secret,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: text },
        },
    }
}

miyu_config::settings! {
    /// DeepSeek 的 key（施工 N-3 上）：没配的照用模型那边配了 DeepSeek 的那一家的 key（设计 36 第二节第 1 条）。
    pub struct DeepseekKey in "net.search.deepseek" {
        /// `{ secret = … }` 或 `{ env = … }`。
        key: Option<Reference> = none {
            kind: secret,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: text },
        },
    }
}

miyu_config::settings! {
    /// Tavily 的 key（施工 N-3 上）。
    pub struct TavilyKey in "net.search.tavily" {
        /// `{ secret = … }` 或 `{ env = … }`。
        key: Option<Reference> = none {
            kind: secret,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: text },
        },
    }
}

miyu_config::settings! {
    /// Brave Search 的 key（施工 N-3 上）。
    pub struct BraveKey in "net.search.brave" {
        /// `{ secret = … }` 或 `{ env = … }`。
        key: Option<Reference> = none {
            kind: secret,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: text },
        },
    }
}

miyu_config::settings! {
    /// 自己搭的 SearXNG（施工 N-3 上）：实例的地址，要开着 JSON 格式。
    pub struct SearxngAddress in "net.search.searxng" {
        /// 例如 `http://127.0.0.1:8888`。
        url: Option<Address> = none {
            kind: url,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "packages", group: "net", control: text },
        },
    }
}

/// 联网这个包的几组配置项，照声明的先后：核心照它登记（常量，核心的清单是常量数组）。
pub const ITEMS: [&[miyu_config::Item]; 6] = [
    SearchSettings::ITEMS,
    ExaKey::ITEMS,
    DeepseekKey::ITEMS,
    TavilyKey::ITEMS,
    BraveKey::ITEMS,
    SearxngAddress::ITEMS,
];

/// 不挑、照出厂顺序的那一个选项。
pub const AUTO: &str = "auto";

#[cfg(test)]
mod tests;

/// 照这一轮的配置 `settings` 拼这一次用得上的（施工 N-3 上）：挑的那一家，配了的 key（照引用取，取不到的当没配；密钥本身
/// 已经去掉前后空白、不会是空的），
/// SearXNG 的地址。DeepSeek 没单独配 key 的，照用模型那边配了 DeepSeek 的那一家供应商的 key（设计 36 第二节第 1 条）：
/// 地址里有 `api.deepseek.com`，或者对的目录是 `deepseek`。
pub(crate) fn using(settings: &dyn miyu_tool::SettingsPort) -> crate::search::Using {
    let values = settings.values();
    let secret = |reference: Option<Reference>| {
        reference
            .and_then(|reference| settings.secret(&reference))
            .map(|secret| secret.expose().to_string())
    };
    let mut using = crate::search::Using {
        preferred: Some(SearchSettings::from(&values).source),
        ..crate::search::Using::default()
    };
    let keys = [
        ("exa", ExaKey::from(&values).key),
        ("deepseek", DeepseekKey::from(&values).key),
        ("tavily", TavilyKey::from(&values).key),
        ("brave", BraveKey::from(&values).key),
    ];
    for (source, reference) in keys {
        if let Some(key) = secret(reference) {
            using.keys.insert(source.to_string(), key);
        }
    }
    if !using.keys.contains_key("deepseek")
        && let Some(key) = secret(deepseek_provider(&values))
    {
        using.keys.insert("deepseek".to_string(), key);
    }
    using.address = match SearxngAddress::from(&values).url {
        Some(Address::Literal(url)) => Some(url).filter(|url| !url.trim().is_empty()),
        Some(Address::Env(name)) => secret(Some(Reference::Env(name))),
        None => None,
    };
    using
}

/// 模型那边配了 DeepSeek 的第一家供应商的 key 引用：`providers.<id>.base_url` 里有 `api.deepseek.com`，或者
/// `providers.<id>.catalog` 是 `deepseek`。
fn deepseek_provider(values: &miyu_config::Values) -> Option<Reference> {
    let text = |key: &str| match values.get(key) {
        Some(miyu_config::Value::Text(text)) => Some(text.to_string()),
        _ => None,
    };
    let ids: Vec<String> = values
        .keys()
        .filter_map(|key| key.strip_prefix("providers.")?.strip_suffix(".key"))
        .map(str::to_string)
        .collect();
    ids.into_iter().find_map(|id| {
        let base = text(&format!("providers.{id}.base_url")).unwrap_or_default();
        let catalog = text(&format!("providers.{id}.catalog")).unwrap_or_default();
        if !(base.contains("api.deepseek.com") || catalog == "deepseek") {
            return None;
        }
        match values.get(&format!("providers.{id}.key")) {
            Some(miyu_config::Value::Secret(reference)) => Some(reference.clone()),
            _ => None,
        }
    })
}

/// 来源少配了什么（施工 N-3 上）：设置页的下拉里那一家后面写「要 key」「要地址」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// 要 key，没配。
    Key,
    /// 要地址，没配。
    Address,
}

/// 选项 `source` 照最终值 `values` 少配了什么：只看写没写，不取密钥本身。DeepSeek 照模型那边的供应商也算配了。和
/// `search.json` 里每家的 `needs` 一致（`settings/tests.rs` 查）。
pub fn missing(source: &str, values: &miyu_config::Values) -> Option<Missing> {
    let set = match source {
        "deepseek" => {
            DeepseekKey::from(values).key.is_some() || deepseek_provider(values).is_some()
        }
        "tavily" => TavilyKey::from(values).key.is_some(),
        "brave" => BraveKey::from(values).key.is_some(),
        "searxng" => {
            return SearxngAddress::from(values)
                .url
                .is_none()
                .then_some(Missing::Address);
        }
        _ => return None,
    };
    (!set).then_some(Missing::Key)
}
