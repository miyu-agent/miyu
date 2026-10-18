//! 读 `software/net/search.json`（施工 N-2 上，设计 36 第二节）：没配置时的顺序、每次几条、一条摘录至多几个字、时限、冷却的台阶、
//! 各家的地址。多一格、少一格都不认，数是 0 的也不认；顺序里写了没列的来源、来源的种类不认得的读不了。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// 来源是哪一种：照它挑请求怎么写、回应怎么读。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// Exa 的 MCP（`web_search_exa`）。
    Exa,
    /// Parallel 的 MCP（`web_search`）。
    Parallel,
    /// Firecrawl 的 MCP（`firecrawl_search`）。
    Firecrawl,
    /// You.com 的 MCP（`you-search`）。
    You,
    /// AnySearch 的 REST 接口。
    Anysearch,
    /// DeepSeek 的搜索接口（施工 N-3 上，要 key）。
    Deepseek,
    /// Tavily（要 key）。
    Tavily,
    /// Brave Search（要 key）。
    Brave,
    /// 自己搭的 SearXNG（要地址）。
    Searxng,
    /// DuckDuckGo 的 HTML 结果页（施工 N-3 下，免 key 的网页抓取兜底）。
    Duckduckgo,
    /// Bing 的结果页（施工 N-3 下）。
    Bing,
}

/// 一家要什么才能用（施工 N-3 上）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Needs {
    /// 什么都不要。
    Nothing,
    /// 不要也能用，配了 key 的走自己的额度（Exa）。
    KeyOptional,
    /// 要 key：没配的跳过。
    Key,
    /// 要地址（SearXNG）：没配的跳过。
    Address,
}

/// 一家来源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Source {
    /// 给人看的名字：时间线上那一句写它。
    pub(crate) name: String,
    /// 是哪一种。
    pub(crate) kind: Kind,
    /// 地址：要地址的那一种是空的，照设置。
    pub(crate) url: String,
    /// 要什么才能用。
    pub(crate) needs: Needs,
    /// 认不认查询里写 `site:`：认的把只要的域名写进查询。
    pub(crate) site_operator: bool,
}

/// 读成的规矩。
#[derive(Debug, Clone)]
pub(crate) struct Rules {
    /// 没配置时的顺序：来源的编号。
    pub(crate) order: Vec<String>,
    /// 每次至多交几条。
    pub(crate) results: usize,
    /// 一条摘录至多几个字。
    pub(crate) excerpt_chars: usize,
    /// 一次请求的时限。
    pub(crate) timeout: Duration,
    /// 冷却的台阶。
    pub(crate) cooldown: Vec<Duration>,
    /// 抓结果页的两家带的浏览器标识（施工 N-3 下）：不带的直接被挡。
    pub(crate) browser_user_agent: String,
    /// 编号 → 来源。
    pub(crate) sources: BTreeMap<String, Source>,
}

/// 文件里的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    order: Vec<String>,
    results: usize,
    excerpt_chars: usize,
    timeout_seconds: u64,
    cooldown_minutes: Vec<u64>,
    browser_user_agent: String,
    sources: BTreeMap<String, Source>,
}

/// 读不成：哪个文件、为什么。
#[derive(Debug)]
pub(crate) struct RulesError {
    file: PathBuf,
    why: String,
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

/// 资源目录 `resources` 里的那一份。
fn file(resources: &Path) -> PathBuf {
    resources.join("software").join("net").join("search.json")
}

/// 读资源目录 `resources` 里的规矩。
pub(crate) fn load(resources: &Path) -> Result<Rules, RulesError> {
    let path = file(resources);
    let bad = |why: String| RulesError {
        file: path.clone(),
        why,
    };
    let text = std::fs::read_to_string(&path).map_err(|error| bad(error.to_string()))?;
    let parsed: File = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
    if parsed.results == 0 || parsed.excerpt_chars == 0 || parsed.timeout_seconds == 0 {
        return Err(bad(
            "results, excerpt_chars and timeout_seconds cannot be 0".to_string(),
        ));
    }
    if parsed.cooldown_minutes.is_empty() || parsed.cooldown_minutes.contains(&0) {
        return Err(bad(
            "cooldown_minutes needs at least one step and no 0".to_string()
        ));
    }
    if let Some((id, _)) = parsed
        .sources
        .iter()
        .find(|(_, source)| (source.needs == Needs::Address) != source.url.is_empty())
    {
        return Err(bad(format!(
            "source {id} must have a url unless it needs an address"
        )));
    }
    if let Some(missing) = parsed
        .order
        .iter()
        .find(|id| !parsed.sources.contains_key(*id))
    {
        return Err(bad(format!("order names an unknown source {missing}")));
    }
    Ok(Rules {
        order: parsed.order,
        results: parsed.results,
        excerpt_chars: parsed.excerpt_chars,
        timeout: Duration::from_secs(parsed.timeout_seconds),
        cooldown: parsed
            .cooldown_minutes
            .iter()
            .map(|minutes| Duration::from_secs(minutes * 60))
            .collect(),
        browser_user_agent: parsed.browser_user_agent,
        sources: parsed.sources,
    })
}

#[cfg(test)]
mod tests;
