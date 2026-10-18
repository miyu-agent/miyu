//! 各家怎么问、怎么读（施工 N-2 上，2026-10-18 照各家的实测写）：MCP 的四家交回的是一段字（Exa 一条条 `Title:`、`URL:`，
//! 别的三家是 JSON 写成的字），AnySearch 是 REST 回 JSON。读成一样的 [`Hit`]：标题、网址、摘录、日期。
//!
//! 限额怎么认在 [`super::Failure::from_text`]：HTTP 429，或者原话里说了限额、额度。

use serde_json::{Value, json};

use super::rules::Kind;
use super::{Failure, Hit};

/// 走 MCP 的几家调哪件工具、参数怎么写。`want` 是要几条，`session` 是这个核心的一个固定编号（Parallel 照它算免费额度）。
pub(crate) fn mcp_call(
    kind: Kind,
    query: &str,
    want: usize,
    session: &str,
) -> Option<(&'static str, Value)> {
    Some(match kind {
        Kind::Exa => (
            "web_search_exa",
            json!({"query": query, "numResults": want, "objective": query}),
        ),
        Kind::Parallel => (
            "web_search",
            json!({"objective": query, "search_queries": [query], "session_id": session}),
        ),
        Kind::Firecrawl => ("firecrawl_search", json!({"query": query, "limit": want})),
        Kind::You => ("you-search", json!({"query": query, "count": want})),
        Kind::Anysearch
        | Kind::Deepseek
        | Kind::Tavily
        | Kind::Brave
        | Kind::Searxng
        | Kind::Duckduckgo
        | Kind::Bing => return None,
    })
}

/// 一次 REST 请求怎么发（施工 N-3 上）：方法、地址（带好查询参数）、另加的头、JSON 正文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rest {
    /// 是 GET；不是的 POST。
    pub(crate) get: bool,
    /// 地址。
    pub(crate) url: String,
    /// 另加的头：认证这些。
    pub(crate) headers: Vec<(&'static str, String)>,
    /// POST 的正文。
    pub(crate) body: Option<Value>,
}

/// 走 REST 的几家怎么问：`url` 是这一家的地址（SearXNG 是设置里的实例地址），`key` 是配的 key（要 key 的几家没有的走不到
/// 这里），`browser` 是抓结果页的两家带的浏览器标识（施工 N-3 下）。走 MCP 的几家没有。
pub(crate) fn rest(
    kind: Kind,
    url: &str,
    query: &str,
    want: usize,
    key: Option<&str>,
    browser: &str,
) -> Option<Rest> {
    let bearer = || {
        key.map(|key| vec![("Authorization", format!("Bearer {key}"))])
            .unwrap_or_default()
    };
    Some(match kind {
        Kind::Anysearch => Rest {
            get: false,
            url: url.to_string(),
            headers: Vec::new(),
            // 它的上限是 10 条。
            body: Some(json!({"query": query, "max_results": want.min(10)})),
        },
        Kind::Deepseek => Rest {
            get: false,
            url: url.to_string(),
            headers: bearer(),
            // 查询至多 128 个字（官方的规矩），条数 1 到 50。
            body: Some(
                json!({"query": query.chars().take(128).collect::<String>(), "count": want.clamp(1, 50)}),
            ),
        },
        Kind::Tavily => Rest {
            get: false,
            url: url.to_string(),
            headers: bearer(),
            body: Some(json!({"query": query, "max_results": want.min(20)})),
        },
        Kind::Brave => Rest {
            get: true,
            url: with_query(url, &[("q", query), ("count", &want.min(20).to_string())]),
            headers: key
                .map(|key| {
                    vec![
                        ("X-Subscription-Token", key.to_string()),
                        ("Accept", "application/json".to_string()),
                    ]
                })
                .unwrap_or_default(),
            body: None,
        },
        Kind::Searxng => Rest {
            get: true,
            url: with_query(
                &format!("{}/search", url.trim_end_matches('/')),
                &[("q", query), ("format", "json")],
            ),
            headers: Vec::new(),
            body: None,
        },
        Kind::Duckduckgo => Rest {
            get: true,
            url: with_query(url, &[("q", query)]),
            headers: vec![("User-Agent", browser.to_string())],
            body: None,
        },
        Kind::Bing => {
            // 照查询的语言挑市场：Bing 照 IP 挑地区，英文的查询在别的地区常给出不相干的结果（2026-10-19 实测）。
            let (market, language) = if query.chars().any(cjk) {
                ("zh-CN", "zh")
            } else {
                ("en-US", "en")
            };
            Rest {
                get: true,
                url: with_query(url, &[("q", query), ("mkt", market), ("setlang", language)]),
                headers: vec![("User-Agent", browser.to_string())],
                body: None,
            }
        }
        Kind::Exa | Kind::Parallel | Kind::Firecrawl | Kind::You => return None,
    })
}

/// 中日韩的字：汉字、假名、谚文。
fn cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{f900}'..='\u{faff}')
}

/// 地址接上查询参数（照 URL 的规矩编码）；读不成地址的照原样接。
fn with_query(url: &str, pairs: &[(&str, &str)]) -> String {
    match reqwest::Url::parse_with_params(url, pairs) {
        Ok(url) => url.to_string(),
        Err(_) => url.to_string(),
    }
}

/// 读一家交回的字。
pub(crate) fn read(kind: Kind, text: &str) -> Result<Vec<Hit>, Failure> {
    match kind {
        Kind::Exa => Ok(exa(text)),
        Kind::Parallel => {
            let value = json(text)?;
            Ok(list(&value["results"])
                .map(|one| {
                    hit(
                        one,
                        "url",
                        "title",
                        joined(&one["excerpts"]),
                        date(&one["publish_date"]),
                    )
                })
                .collect())
        }
        Kind::Firecrawl => {
            let value = json(text)?;
            let web = if value["data"].is_array() {
                &value["data"]
            } else {
                &value["data"]["web"]
            };
            Ok(list(web)
                .map(|one| hit(one, "url", "title", string(&one["description"]), None))
                .collect())
        }
        Kind::You => {
            let value = json(text)?;
            Ok(list(&value["results"]["web"])
                .map(|one| {
                    let highlights = joined(&one["contents"]["highlights"]);
                    let snippet = highlights.or_else(|| string(&one["description"]));
                    hit(one, "url", "title", snippet, date(&one["page_age"]))
                })
                .collect())
        }
        Kind::Deepseek => {
            let value = json(text)?;
            Ok(list(&value["results"])
                .map(|one| {
                    let snippet = string(&one["summary"]).or_else(|| string(&one["snippet"]));
                    hit(one, "url", "title", snippet, date(&one["date"]))
                })
                .collect())
        }
        Kind::Tavily => {
            let value = json(text)?;
            Ok(list(&value["results"])
                .map(|one| {
                    hit(
                        one,
                        "url",
                        "title",
                        string(&one["content"]),
                        date(&one["published_date"]),
                    )
                })
                .collect())
        }
        Kind::Brave => {
            let value = json(text)?;
            Ok(list(&value["web"]["results"])
                .map(|one| {
                    hit(
                        one,
                        "url",
                        "title",
                        string(&one["description"]),
                        date(&one["page_age"]),
                    )
                })
                .collect())
        }
        Kind::Searxng => {
            let value = json(text)?;
            Ok(list(&value["results"])
                .map(|one| {
                    hit(
                        one,
                        "url",
                        "title",
                        string(&one["content"]),
                        date(&one["publishedDate"]),
                    )
                })
                .collect())
        }
        Kind::Duckduckgo => super::scrape::duckduckgo(text),
        Kind::Bing => super::scrape::bing(text),
        Kind::Anysearch => {
            let value = json(text)?;
            if value["code"].as_i64() != Some(0) {
                let said = value["message"].as_str().unwrap_or("not ok");
                return Err(Failure::from_text(said));
            }
            Ok(list(&value["data"]["results"])
                .map(|one| {
                    let snippet = string(&one["content"]).or_else(|| string(&one["snippet"]));
                    hit(one, "url", "title", snippet, None)
                })
                .collect())
        }
    }
}

/// Exa 的字：一条从 `Title: ` 开始，下一行 `URL: `；`Published: N/A` 是没有日期；`Highlights:` 往后到下一条是摘录。
fn exa(text: &str) -> Vec<Hit> {
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&at| {
            lines[at].starts_with("Title: ")
                && lines
                    .get(at + 1)
                    .is_some_and(|next| next.starts_with("URL: "))
        })
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &start)| {
            let end = starts.get(n + 1).copied().unwrap_or(lines.len());
            let record = &lines[start..end];
            let field = |name: &str| {
                record
                    .iter()
                    .find_map(|line| line.strip_prefix(name))
                    .map(str::trim)
                    .filter(|value| !value.is_empty() && *value != "N/A")
                    .map(str::to_string)
            };
            let snippet = record
                .iter()
                .position(|line| line.trim() == "Highlights:" || line.trim() == "Text:")
                .map(|at| record[at + 1..].join("\n").trim().to_string())
                .filter(|text| !text.is_empty());
            Hit {
                title: field("Title: ").unwrap_or_default(),
                url: field("URL: ").unwrap_or_default(),
                snippet,
                date: field("Published: ").and_then(|text| iso_day(&text)),
            }
        })
        .filter(|hit| !hit.url.is_empty())
        .collect()
}

/// 读成 JSON：读不懂的当失败。
fn json(text: &str) -> Result<Value, Failure> {
    serde_json::from_str(text.trim()).map_err(|error| {
        Failure::from_text(&format!(
            "{error}: {}",
            text.chars().take(200).collect::<String>()
        ))
    })
}

/// 一个数组里的几项：不是数组的当空的。
fn list(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

/// 一条：网址、标题照给的键取，网址没有的后面会被筛掉。
fn hit(one: &Value, url: &str, title: &str, snippet: Option<String>, date: Option<String>) -> Hit {
    Hit {
        title: string(&one[title]).unwrap_or_default(),
        url: string(&one[url]).unwrap_or_default(),
        snippet,
        date,
    }
}

/// 不空的字。
fn string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// 一串字接起来。
fn joined(value: &Value) -> Option<String> {
    let parts: Vec<&str> = list(value)
        .filter_map(Value::as_str)
        .map(str::trim)
        .collect();
    let text = parts.join("\n");
    (!text.trim().is_empty()).then_some(text)
}

/// 日期：`2026-07-09` 这样开头的只要前十个字（`2026-07-09T12:00:00` 取 `2026-07-09`）；别的写法（`Mon, 01 Jan 2024`）不要。
fn date(value: &Value) -> Option<String> {
    string(value).and_then(|text| iso_day(&text))
}

/// 开头是 `YYYY-MM-DD` 的取这十个字。
fn iso_day(text: &str) -> Option<String> {
    let day: String = text.chars().take(10).collect();
    let shape = day.char_indices().all(|(at, c)| match at {
        4 | 7 => c == '-',
        _ => c.is_ascii_digit(),
    });
    (day.len() == 10 && shape).then_some(day)
}

#[cfg(test)]
mod tests;
