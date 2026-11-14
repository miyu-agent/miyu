//! 代抓网页（施工 N-4，设计 36 的 N-4）：`web_fetch` 自己抓不下来的（挡爬虫、要跑脚本），交给能抓页面的几家，照 `search.json`
//! 的 `fetch` 一家家试。和搜索共用一份家底：冷却、MCP 的会话、客户端都是同一份，哪家报了限额，搜和抓一起歇。
//!
//! 2026-10-19 照实测写各家怎么问、怎么读：Exa 的 `web_fetch_exa` 交回一段字（`# 标题`、`URL:` 这些开头几行，再是正文）；
//! Firecrawl 的 `firecrawl_scrape` 交回 JSON（`markdown`、`metadata.title`）；Parallel 的 `web_fetch` 交回 JSON 的 `results`、
//! `errors`，要全文得说 `full_content`。正文少于 `min_chars` 个字的（登录墙、跳转页）当这一家没抓到，换下一家，不冷却。
//!
//! 选了带代抓的联网服务的（施工 NF-1，项目主人 2026-11-14 定）：配了 key 的，`web_fetch` 自己抓之前先交它
//! （[`Search::fetch_by_provider`]）；它调不通（限额、余额不足、服务忙、超时、太短）再自己抓，再照自动的名单（[`Search::fetch`]）。
//! DeepSeek Fetch 是 REST：`POST {"url"}`，回 `{"title", "content"}`（Markdown）。

use std::time::Instant;

use reqwest::Url;
use serde_json::{Value, json};

use super::rules::{Kind, Source};
use super::sources::Rest;
use super::{Failure, Missed, Search, Using};
use crate::TARGET;
use crate::fetch::clients::Way;

/// 代抓到的一页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// 是哪一家：给人看的名字。
    pub source: String,
    /// 标题，没有的没有。
    pub title: Option<String>,
    /// 正文：Markdown。
    pub text: String,
    /// 超了 `max_chars`、截了后面。
    pub cut: bool,
}

impl Search {
    /// 选了带代抓的联网服务的（施工 NF-1 补，项目主人 2026-11-14 定）：`web_fetch` 自己抓之前先交它。没选、选了只管搜索的、
    /// 要 key 没配的不算。
    pub fn has_provider(&self, using: &Using) -> bool {
        self.ready()
            .is_some_and(|ready| ready.provider(using).is_some())
    }

    /// 交选了的联网服务代抓 `url`（见 [`Search::has_provider`]），冷却着的不问。
    ///
    /// # Errors
    ///
    /// 见 [`Missed`]：没有这样的服务、冷却着的是 [`Missed::Cooling`]。
    pub async fn fetch_by_provider(&self, url: &str, using: &Using) -> Result<Fetched, Missed> {
        let ready = self.ready().ok_or(Missed::NotReady)?;
        let provider = ready.provider(using).into_iter().cloned().collect();
        ready.fetch_from(provider, url, using).await
    }

    /// 照自动的名单（`fetch` 的先后）代抓 `url`，冷却着的跳过；选了带代抓的服务的，它已经先问过，不再问。
    ///
    /// # Errors
    ///
    /// 见 [`Missed`]。
    pub async fn fetch(&self, url: &str, using: &Using) -> Result<Fetched, Missed> {
        let ready = self.ready().ok_or(Missed::NotReady)?;
        let asked = ready.provider(using);
        let order = ready.rules.fetch.order.iter();
        let ids = order.filter(|id| Some(*id) != asked).cloned().collect();
        ready.fetch_from(ids, url, using).await
    }
}

impl super::Ready {
    /// 选了的那一家能抓页面、用得上（要 key 的配了）的：它的编号。
    fn provider<'a>(&self, using: &'a Using) -> Option<&'a String> {
        using.preferred.as_ref().filter(|id| {
            self.rules.sources.get(*id).is_some_and(Source::fetches) && self.usable(id, using)
        })
    }

    /// 照 `ids` 的先后一家家试，冷却着的跳过；报限额的进冷却。
    async fn fetch_from(
        &self,
        ids: Vec<String>,
        url: &str,
        using: &Using,
    ) -> Result<Fetched, Missed> {
        let mut last = None;
        for id in &ids {
            let source = &self.rules.sources[id];
            if self.cooling(id).is_some() {
                continue;
            }
            match self.page(id, source, url, using).await {
                Ok(fetched) => {
                    self.cooldown().succeeded(id);
                    return Ok(fetched);
                }
                Err(Failure::Limited) => {
                    self.cooldown().limited(id, Instant::now());
                    tracing::warn!(target: TARGET, source = id.as_str(), "web fetch source limited");
                    last = Some(format!("{} is rate limited", source.name));
                }
                Err(Failure::Failed(why)) => {
                    tracing::warn!(target: TARGET, source = id.as_str(), why = why.as_str(), "web fetch source failed");
                    last = Some(format!("{}: {why}", source.name));
                }
                Err(Failure::Gone) => last = Some(format!("{}: the session expired", source.name)),
            }
        }
        Err(last.map_or(Missed::Cooling, Missed::Failed))
    }
}

impl Search {
    /// 自己抓到的正文 `text` 短得像登录墙、跳转页（少于 `fetch.min_chars` 个字）：值得再请别家试试。规矩读不懂的不算。
    pub fn too_short(&self, text: &str) -> bool {
        self.ready()
            .is_some_and(|ready| text.trim().chars().count() < ready.rules.fetch.min_chars)
    }
}

impl super::Ready {
    /// 叫 `id` 这一家抓 `url`：另写了抓页面地址的（DeepSeek）照 REST 问，key 放在头里；别的走 MCP，配了 key 的 Exa 接在
    /// 地址后面。
    async fn page(
        &self,
        id: &str,
        source: &Source,
        url: &str,
        using: &Using,
    ) -> Result<Fetched, Failure> {
        let mut endpoint = Url::parse(source.fetch_url.as_deref().unwrap_or(&source.url))
            .map_err(|error| Failure::Failed(error.to_string()))?;
        if let (Kind::Exa, Some(key)) = (source.kind, using.key(id)) {
            endpoint.query_pairs_mut().append_pair("exaApiKey", key);
        }
        let way = self.proxies.via(&endpoint).map_or(Way::Direct, Way::Proxy);
        let client = self
            .clients
            .get(&way)
            .map_err(|error| Failure::Failed(error.without_url().to_string()))?;
        let fetching = &self.rules.fetch;
        let answer = if source.fetch_url.is_some() {
            let rest = rest(endpoint.as_str(), url, using.key(id))?;
            super::rest::send(&client, &rest, fetching.timeout).await?
        } else {
            let (tool, args) = call(source.kind, url, fetching.max_chars, &self.session)
                .ok_or_else(|| Failure::Failed("this source cannot fetch pages".to_string()))?;
            super::mcp::call(
                &client,
                &self.sessions,
                endpoint.as_str(),
                tool,
                args,
                fetching.timeout,
            )
            .await?
        };
        let (title, text) = read(source.kind, &answer)?;
        let text = text.trim();
        let chars = text.chars().count();
        if chars < fetching.min_chars {
            return Err(Failure::Failed(format!(
                "only {chars} characters came back"
            )));
        }
        let (text, cut) = match text.char_indices().nth(fetching.max_chars) {
            Some((end, _)) => (text[..end].to_string(), true),
            None => (text.to_string(), false),
        };
        Ok(Fetched {
            source: source.name.clone(),
            title: title.filter(|title| !title.trim().is_empty()),
            text,
            cut,
        })
    }
}

/// 这一种调哪件工具、参数怎么写：`chars` 是至多要几个字，`session` 是这个核心的固定编号（Parallel 照它算免费额度）。
pub(crate) fn call(
    kind: Kind,
    url: &str,
    chars: usize,
    session: &str,
) -> Option<(&'static str, Value)> {
    Some(match kind {
        Kind::Exa => (
            "web_fetch_exa",
            json!({"urls": [url], "maxCharacters": chars}),
        ),
        Kind::Firecrawl => (
            "firecrawl_scrape",
            json!({"url": url, "formats": ["markdown"], "onlyMainContent": true}),
        ),
        Kind::Parallel => (
            "web_fetch",
            json!({"urls": [url], "full_content": true, "session_id": session}),
        ),
        _ => return None,
    })
}

/// DeepSeek Fetch 的一次请求：`endpoint` 是抓页面的地址，`url` 是要抓的，`key` 放在 `Authorization` 里。官方只收 http、https、
/// 至多 8192 个字、不带用户名密码的网址，不合的不去问。
pub(crate) fn rest(endpoint: &str, url: &str, key: Option<&str>) -> Result<Rest, Failure> {
    let refused = || Failure::Failed("this source cannot fetch this address".to_string());
    let parsed = Url::parse(url).map_err(|_| refused())?;
    if url.chars().count() > 8192
        || !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(refused());
    }
    Ok(Rest {
        get: false,
        url: endpoint.to_string(),
        headers: key
            .map(|key| vec![("Authorization", format!("Bearer {key}"))])
            .unwrap_or_default(),
        body: Some(json!({"url": url})),
    })
}

/// 读回应：标题和正文。
pub(crate) fn read(kind: Kind, answer: &str) -> Result<(Option<String>, String), Failure> {
    match kind {
        Kind::Exa => Ok(exa(answer)),
        Kind::Firecrawl => {
            let value = json(answer)?;
            let markdown = value["markdown"].as_str().unwrap_or_default().to_string();
            Ok((
                value["metadata"]["title"].as_str().map(str::to_string),
                markdown,
            ))
        }
        Kind::Parallel => {
            let value = json(answer)?;
            let Some(first) = value["results"].as_array().and_then(|all| all.first()) else {
                let why = value["errors"][0]["error_type"]
                    .as_str()
                    .unwrap_or("nothing came back");
                return Err(Failure::from_text(why));
            };
            let text = match first["full_content"].as_str() {
                Some(full) => full.to_string(),
                None => first["excerpts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            };
            Ok((first["title"].as_str().map(str::to_string), text))
        }
        Kind::Deepseek => {
            let value = json(answer)?;
            match value["content"].as_str() {
                Some(content) => Ok((
                    value["title"].as_str().map(str::to_string),
                    content.to_string(),
                )),
                None => Err(Failure::from_text(
                    value["error"]["message"]
                        .as_str()
                        .unwrap_or("nothing came back"),
                )),
            }
        }
        _ => Err(Failure::Failed(
            "this source cannot fetch pages".to_string(),
        )),
    }
}

/// 读成 JSON：读不懂的照原话认是不是限额。
fn json(answer: &str) -> Result<Value, Failure> {
    serde_json::from_str(answer).map_err(|_| Failure::from_text(answer))
}

/// Exa 的一段字：第一行 `# 标题`，接着几行 `URL:`、`Published:`、`Author:` 这样的，空一行以后是正文。
fn exa(answer: &str) -> (Option<String>, String) {
    let mut lines = answer.lines().peekable();
    let title = lines
        .peek()
        .and_then(|line| line.strip_prefix("# "))
        .map(|title| title.trim().to_string());
    if title.is_some() {
        lines.next();
    }
    while lines.peek().is_some_and(|line| header(line)) {
        lines.next();
    }
    (title, lines.collect::<Vec<_>>().join("\n"))
}

/// Exa 开头那几行：`URL:`、`Published:` 这样，冒号前面是一个英文词。
fn header(line: &str) -> bool {
    line.split_once(": ").is_some_and(|(name, _)| {
        !name.is_empty() && name.len() <= 12 && name.chars().all(|c| c.is_ascii_alphabetic())
    })
}

#[cfg(test)]
mod tests;
