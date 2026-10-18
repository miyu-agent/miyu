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
        Kind::Anysearch => return None,
    })
}

/// AnySearch 的请求：至多 10 条（它的上限）。
pub(crate) fn anysearch_body(query: &str, want: usize) -> Value {
    json!({"query": query, "max_results": want.min(10)})
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
                date: field("Published: ").map(|text| text.chars().take(10).collect()),
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

/// 日期：只要前十个字（`2026-07-09T12:00:00` 取 `2026-07-09`）。
fn date(value: &Value) -> Option<String> {
    string(value).map(|text| text.chars().take(10).collect())
}

#[cfg(test)]
mod tests;
