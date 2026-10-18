//! 搜网页（施工 N-2 上，设计 36 第二节）：`web_search` 那件工具的里子。
//!
//! - 照 `search.json` 的顺序一家家试：冷却着的跳过；出错的（超时、5xx、读不懂）这一次换下一家；报限额的进冷却
//!   （`cooldown.rs`）再换下一家；成功的清零。都不成的照最后一家的原因报。
//! - 要几条照 `results`，摘录截到 `excerpt_chars` 个字；同一个网址只留一条；`allowed_domains`、`blocked_domains` 照结果的网址
//!   筛，认 `site:` 的几家另把只要的域名写进查询。
//! - 请求固定的几家公开地址，不过地址闸；代理照环境变量，和抓网页同一套（`proxy.rs`），客户端照「怎么连」复用。
//! - 第一次搜才读规矩，读不懂的记一条 `WARN web search not ready`，这个核心的生命周期里不再试。

mod cooldown;
mod mcp;
mod rest;
mod rules;
mod sources;
mod using;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::Url;

use crate::TARGET;
use crate::fetch::clients::{Clients, Way};
use crate::proxy::Proxies;
use cooldown::Cooldowns;
use rules::{Kind, Needs, Rules, Source};

pub use using::Using;

/// 搜到的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 标题。
    pub title: String,
    /// 网址。
    pub url: String,
    /// 摘录，没有的没有。
    pub snippet: Option<String>,
    /// 发布日期，`2026-10-18` 这样的前十个字；没有的没有。
    pub date: Option<String>,
}

/// 要搜的。
#[derive(Debug, Clone, Default)]
pub struct Query {
    /// 搜什么。
    pub text: String,
    /// 只要这几个域名的（含子域名），空的是不限。
    pub allowed: Vec<String>,
    /// 不要这几个域名的（含子域名）。
    pub blocked: Vec<String>,
}

/// 搜成了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 是哪一家：给人看的名字。
    pub source: String,
    /// 几条，可以是空的。
    pub hits: Vec<Hit>,
}

/// 搜不成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missed {
    /// 都试过了，最后一家的原因（英文的一句）。
    Failed(String),
    /// 都在冷却，一家都没试。
    Cooling,
    /// 规矩文件读不懂。
    NotReady,
}

/// 一家这一次没成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// 报了限额。
    Limited,
    /// 别的：原因是一句英文。
    Failed(String),
    /// MCP 的会话过期了（只在 `mcp.rs` 里面，重新握手）。
    Gone,
}

impl Failure {
    /// 照对方的原话认：说了限额、额度的是 [`Failure::Limited`]。
    pub(crate) fn from_text(text: &str) -> Failure {
        let lower = text.to_ascii_lowercase();
        let limited = [
            "rate limit",
            "rate-limit",
            "ratelimit",
            "too many requests",
            "quota",
            "credit",
            "usage limit",
            "limit reached",
            "http 429",
            "http 402",
            "insufficient balance",
        ];
        if limited.iter().any(|word| lower.contains(word)) {
            Failure::Limited
        } else {
            Failure::Failed(text.chars().take(200).collect())
        }
    }
}

/// 搜网页的家底：一个核心一份，冷却、MCP 的会话都记在这里。
pub struct Search {
    resources: PathBuf,
    ready: OnceLock<Result<Ready, ()>>,
    #[cfg(any(test, feature = "testkit"))]
    urls: Vec<(String, String)>,
    #[cfg(any(test, feature = "testkit"))]
    only: Vec<String>,
    #[cfg(any(test, feature = "testkit"))]
    ladder: Vec<std::time::Duration>,
}

/// 读了规矩以后备好的。
struct Ready {
    rules: Rules,
    clients: Clients,
    proxies: Proxies,
    sessions: mcp::Sessions,
    cooldowns: Mutex<Cooldowns>,
    /// 这个核心的一个固定编号：Parallel 照它算免费额度。
    session: String,
}

impl Search {
    /// 一份家底：只记资源目录在哪，不碰磁盘。
    pub fn new(resources: &Path) -> Search {
        Search {
            resources: resources.to_path_buf(),
            ready: OnceLock::new(),
            #[cfg(any(test, feature = "testkit"))]
            urls: Vec::new(),
            #[cfg(any(test, feature = "testkit"))]
            only: Vec::new(),
            #[cfg(any(test, feature = "testkit"))]
            ladder: Vec::new(),
        }
    }

    /// 测试用：几家的地址换成给的（编号，地址），不走代理。生产的构建里没有这个方法。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn testing(mut self, urls: Vec<(String, String)>) -> Search {
        self.urls = urls;
        self
    }

    /// 测试用：只试这几家，照给的先后（真网络的实测一家家走）。生产的构建里没有这个方法。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn only(mut self, ids: &[&str]) -> Search {
        self.only = ids.iter().map(|id| (*id).to_string()).collect();
        self
    }

    /// 测试用：冷却的台阶换成给的（第一级可以是 0：限额了马上回来，好测「成功清零」）。生产的构建里没有这个方法。
    #[cfg(any(test, feature = "testkit"))]
    #[must_use]
    pub fn cooldown(mut self, ladder: Vec<std::time::Duration>) -> Search {
        self.ladder = ladder;
        self
    }

    /// 搜一次，照 `using`：设置里挑的那一家排在最前，用不了照出厂的顺序兜底（设计 36 第二节第 3 条）；要 key、要地址没配的
    /// 跳过（施工 N-3 上）。
    ///
    /// # Errors
    ///
    /// 见 [`Missed`]。
    pub async fn search(&self, query: &Query, using: &Using) -> Result<Found, Missed> {
        let ready = self.ready().ok_or(Missed::NotReady)?;
        let mut last = None;
        for id in &ready.order(using) {
            let source = &ready.rules.sources[id];
            if ready.cooling(id).is_some() {
                continue;
            }
            match ready.ask(id, source, query, using).await {
                Ok(hits) => {
                    ready.cooldown().succeeded(id);
                    let hits = tidy(hits, query, ready.rules.results, ready.rules.excerpt_chars);
                    return Ok(Found {
                        source: source.name.clone(),
                        hits,
                    });
                }
                Err(Failure::Limited) => {
                    let until = ready.cooldown().limited(id, Instant::now());
                    let minutes = until.saturating_duration_since(Instant::now()).as_secs() / 60;
                    tracing::warn!(target: TARGET, source = id.as_str(), minutes, "web search source limited");
                    last = Some(format!("{} is rate limited", source.name));
                }
                Err(failure) => {
                    let why = match failure {
                        Failure::Failed(why) => why,
                        Failure::Limited | Failure::Gone => "the session expired".to_string(),
                    };
                    tracing::warn!(target: TARGET, source = id.as_str(), why = why.as_str(), "web search source failed");
                    last = Some(format!("{}: {why}", source.name));
                }
            }
        }
        Err(last.map_or(Missed::Cooling, Missed::Failed))
    }

    /// 这一刻冷却着的几家：编号和冷却到什么时候（施工 N-2 下，设置页的下拉里写「冷却到几点」）。还没搜过的（规矩还没读）
    /// 一家都没有，不为这个去读规矩。
    pub fn cooling(&self) -> Vec<(String, SystemTime)> {
        let Some(Ok(ready)) = self.ready.get() else {
            return Vec::new();
        };
        let (now, wall) = (Instant::now(), SystemTime::now());
        ready
            .rules
            .sources
            .keys()
            .filter_map(|id| {
                let until = ready.cooling(id)?;
                Some((id.clone(), wall + until.saturating_duration_since(now)))
            })
            .collect()
    }

    fn ready(&self) -> Option<&Ready> {
        self.ready
            .get_or_init(|| match rules::load(&self.resources) {
                Ok(rules) => {
                    let rules = self.adjusted(rules);
                    Ok(Ready {
                        cooldowns: Mutex::new(Cooldowns::new(rules.cooldown.clone())),
                        rules,
                        clients: Clients::new(std::time::Duration::from_secs(120), 8),
                        proxies: self.proxies(),
                        sessions: mcp::Sessions::default(),
                        session: session_id(),
                    })
                }
                Err(error) => {
                    tracing::warn!(target: TARGET, error = %error, "web search not ready");
                    Err(())
                }
            })
            .as_ref()
            .ok()
    }

    /// 测试的口子换掉的几样：地址、只试哪几家、冷却台阶。
    #[cfg(any(test, feature = "testkit"))]
    fn adjusted(&self, mut rules: Rules) -> Rules {
        for (id, url) in &self.urls {
            if let Some(source) = rules.sources.get_mut(id) {
                source.url.clone_from(url);
            }
        }
        if !self.only.is_empty() {
            rules.order = self.only.clone();
        }
        if !self.ladder.is_empty() {
            rules.cooldown = self.ladder.clone();
        }
        rules
    }

    /// 生产的构建里没有测试的口子，照原样。
    #[cfg(not(any(test, feature = "testkit")))]
    fn adjusted(&self, rules: Rules) -> Rules {
        rules
    }

    /// 代理：平时照环境变量；测试换了地址的不走代理。
    fn proxies(&self) -> Proxies {
        #[cfg(any(test, feature = "testkit"))]
        if !self.urls.is_empty() {
            return Proxies::given(None, "");
        }
        Proxies::from_env()
    }
}

impl Ready {
    /// 这一次试的先后：挑了出厂有的那一家的排在最前，其余照出厂的顺序；要 key、要地址没配的不试（施工 N-3 上）。
    fn order(&self, using: &Using) -> Vec<String> {
        let ready = |id: &String| {
            self.rules
                .sources
                .get(id)
                .is_some_and(|source| match source.needs {
                    Needs::Nothing | Needs::KeyOptional => true,
                    Needs::Key => using.key(id).is_some(),
                    Needs::Address => using.address.is_some(),
                })
        };
        let first = using
            .preferred
            .as_ref()
            .filter(|id| self.rules.sources.contains_key(*id));
        first
            .cloned()
            .into_iter()
            .chain(
                self.rules
                    .order
                    .iter()
                    .filter(|id| Some(*id) != first)
                    .cloned(),
            )
            .filter(ready)
            .collect()
    }

    fn cooldown(&self) -> std::sync::MutexGuard<'_, Cooldowns> {
        self.cooldowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn cooling(&self, id: &str) -> Option<Instant> {
        self.cooldown().until(id, Instant::now())
    }

    /// 问一家 `id`：要地址的照设置里的地址，配了 key 的带上（Exa 接在地址后面，别的几家放在头里）。
    async fn ask(
        &self,
        id: &str,
        source: &Source,
        query: &Query,
        using: &Using,
    ) -> Result<Vec<Hit>, Failure> {
        let key = using.key(id);
        let base = match source.needs {
            Needs::Address => using.address.clone().unwrap_or_default(),
            _ => source.url.clone(),
        };
        let mut url = Url::parse(&base).map_err(|error| Failure::Failed(error.to_string()))?;
        if let (Kind::Exa, Some(key)) = (source.kind, key) {
            url.query_pairs_mut().append_pair("exaApiKey", key);
        }
        let way = self.proxies.via(&url).map_or(Way::Direct, Way::Proxy);
        let client = self
            .clients
            .get(&way)
            .map_err(|error| Failure::Failed(error.without_url().to_string()))?;
        let text = phrase(query, source);
        // 要筛域名的多要几条，筛完还够数。
        let want = if query.allowed.is_empty() && query.blocked.is_empty() {
            self.rules.results
        } else {
            self.rules.results * 2
        };
        let timeout = self.rules.timeout;
        let answer = match sources::mcp_call(source.kind, &text, want, &self.session) {
            Some((tool, args)) => {
                mcp::call(&client, &self.sessions, url.as_str(), tool, args, timeout).await?
            }
            None => {
                let rest = sources::rest(source.kind, url.as_str(), &text, want, key)
                    .ok_or_else(|| Failure::Failed("no way to ask this source".to_string()))?;
                rest::send(&client, &rest, timeout).await?
            }
        };
        sources::read(source.kind, &answer)
    }
}

/// 写给这一家的查询：认 `site:` 的把只要的域名接在后面。
fn phrase(query: &Query, source: &Source) -> String {
    let text = query.text.trim();
    if !source.site_operator || query.allowed.is_empty() {
        return text.to_string();
    }
    let sites: Vec<String> = query
        .allowed
        .iter()
        .map(|domain| format!("site:{}", domain_of(domain)))
        .collect();
    format!("{text} {}", sites.join(" OR "))
}

/// 收拾：去掉没有网址的、重复的、不在只要的域名里的、在不要的域名里的；摘录截短；至多 `most` 条。
fn tidy(hits: Vec<Hit>, query: &Query, most: usize, excerpt: usize) -> Vec<Hit> {
    let allowed: Vec<String> = query
        .allowed
        .iter()
        .map(|domain| domain_of(domain))
        .collect();
    let blocked: Vec<String> = query
        .blocked
        .iter()
        .map(|domain| domain_of(domain))
        .collect();
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|hit| !hit.url.is_empty() && seen.insert(hit.url.clone()))
        .filter(|hit| {
            let host = Url::parse(&hit.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
                .unwrap_or_default();
            let under = |domain: &String| host == *domain || host.ends_with(&format!(".{domain}"));
            (allowed.is_empty() || allowed.iter().any(under)) && !blocked.iter().any(under)
        })
        .take(most)
        .map(|mut hit| {
            hit.snippet = hit.snippet.map(|text| clip(&text, excerpt));
            hit
        })
        .collect()
}

/// 她写的域名收成主机名：去掉前后空白、`http(s)://`、路径、开头的 `www.`、`*.`，小写。
fn domain_of(written: &str) -> String {
    let lower = written.trim().to_ascii_lowercase();
    let bare = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or_default();
    bare.trim_start_matches("*.")
        .trim_start_matches("www.")
        .to_string()
}

/// 截到 `most` 个字，截了的接一个省略号。
fn clip(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((end, _)) => format!("{}…", text[..end].trim_end()),
        None => text.to_string(),
    }
}

/// 这个核心的一个固定编号：进程号和起来的时刻拼的 32 位十六进制。
fn session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("{:016x}{:016x}", std::process::id(), nanos as u64)
}

#[cfg(test)]
mod tests;
