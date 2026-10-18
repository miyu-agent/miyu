//! 各家交回的样子读成一样的几条（样本照 2026-10-18 实测的回应删短）。

use super::{read, rest};
use crate::search::rules::Kind;
use crate::search::{Failure, Hit};

fn hit(title: &str, url: &str, snippet: Option<&str>, date: Option<&str>) -> Hit {
    Hit {
        title: title.to_string(),
        url: url.to_string(),
        snippet: snippet.map(str::to_string),
        date: date.map(str::to_string),
    }
}

#[test]
fn exa_records_are_split_on_title_and_url_lines() {
    let text = "Title: Rust 1.97.0\nURL: https://github.com/rust-lang/rust/releases/tag/1.97.0\nPublished: N/A\nAuthor: N/A\nHighlights:\n# Rust 1.97.0\n\n- Tag: 1.97.0\n\nTitle: Announcing Rust\nURL: https://blog.rust-lang.org/\nPublished: 2026-07-09T00:00:00.000Z\nAuthor: The Rust Team\nHighlights:\nThe Rust team is happy\n";
    assert_eq!(
        read(Kind::Exa, text).unwrap(),
        [
            hit(
                "Rust 1.97.0",
                "https://github.com/rust-lang/rust/releases/tag/1.97.0",
                Some("# Rust 1.97.0\n\n- Tag: 1.97.0"),
                None
            ),
            hit(
                "Announcing Rust",
                "https://blog.rust-lang.org/",
                Some("The Rust team is happy"),
                Some("2026-07-09")
            ),
        ]
    );
    assert!(read(Kind::Exa, "No results found.").unwrap().is_empty());
}

#[test]
fn the_json_sources_are_read_into_the_same_hits() {
    let parallel = r#"{"search_id":"s","results":[{"url":"https://doc.rust-lang.org/","title":"Release Notes","publish_date":"2026-10-15","excerpts":["one","two"]}]}"#;
    assert_eq!(
        read(Kind::Parallel, parallel).unwrap(),
        [hit(
            "Release Notes",
            "https://doc.rust-lang.org/",
            Some("one\ntwo"),
            Some("2026-10-15")
        )]
    );
    let firecrawl = r#"{"success":true,"data":{"web":[{"url":"https://blog.rust-lang.org/","title":"Announcing","description":"rustup update","position":1}]}}"#;
    assert_eq!(
        read(Kind::Firecrawl, firecrawl).unwrap(),
        [hit(
            "Announcing",
            "https://blog.rust-lang.org/",
            Some("rustup update"),
            None
        )]
    );
    let you = r#"{"results":{"web":[{"url":"https://releases.rs/","title":"1.97.0","description":"short","page_age":"2026-05-22T00:00:00","contents":{"highlights":["long one","long two"]}}]}}"#;
    assert_eq!(
        read(Kind::You, you).unwrap(),
        [hit(
            "1.97.0",
            "https://releases.rs/",
            Some("long one\nlong two"),
            Some("2026-05-22")
        )],
        "有 highlights 的用它"
    );
    let anysearch = r#"{"code":0,"message":"success","data":{"results":[{"title":"select","url":"https://docs.rs/tokio","snippet":"short","content":"longer content"}]}}"#;
    assert_eq!(
        read(Kind::Anysearch, anysearch).unwrap(),
        [hit(
            "select",
            "https://docs.rs/tokio",
            Some("longer content"),
            None
        )]
    );
}

#[test]
fn a_refusal_or_garbage_is_a_failure_and_quota_words_mean_limited() {
    assert_eq!(
        read(
            Kind::Anysearch,
            r#"{"code":429,"message":"Anonymous quota exhausted"}"#
        ),
        Err(Failure::Limited)
    );
    assert!(matches!(
        read(Kind::Parallel, "<html>oops</html>"),
        Err(Failure::Failed(_))
    ));
    assert!(
        read(Kind::You, r#"{"results":{}}"#).unwrap().is_empty(),
        "没有结果的是空的，不算出错"
    );
}

/// 要 key 的几家和 SearXNG 的回应（施工 N-3 上，样本照各家文档写的样子）。日期只收 `YYYY-MM-DD` 开头的。
#[test]
fn the_keyed_sources_are_read_into_the_same_hits() {
    let deepseek = r#"{"results":[{"title":"DeepSeek V4","url":"https://api-docs.deepseek.com/news","snippet":"short","summary":"a longer summary","date":"2026-10-01T08:00:00Z","site_name":"DeepSeek"}]}"#;
    assert_eq!(
        read(Kind::Deepseek, deepseek).unwrap(),
        [hit(
            "DeepSeek V4",
            "https://api-docs.deepseek.com/news",
            Some("a longer summary"),
            Some("2026-10-01")
        )],
        "长的 summary 优先"
    );
    let tavily = r#"{"query":"q","results":[{"title":"T","url":"https://t.example/","content":"snippet","score":0.9,"published_date":"Mon, 01 Jan 2024 12:00:00 GMT"}]}"#;
    assert_eq!(
        read(Kind::Tavily, tavily).unwrap(),
        [hit("T", "https://t.example/", Some("snippet"), None)],
        "不是 YYYY-MM-DD 的日期不要"
    );
    let brave = r#"{"web":{"results":[{"title":"B","url":"https://b.example/","description":"desc","page_age":"2026-09-30T00:00:00"}]}}"#;
    assert_eq!(
        read(Kind::Brave, brave).unwrap(),
        [hit(
            "B",
            "https://b.example/",
            Some("desc"),
            Some("2026-09-30")
        )]
    );
    let searxng = r#"{"query":"q","results":[{"title":"S","url":"https://s.example/","content":"c","publishedDate":null}]}"#;
    assert_eq!(
        read(Kind::Searxng, searxng).unwrap(),
        [hit("S", "https://s.example/", Some("c"), None)]
    );
}

/// 几家怎么问（施工 N-3 上）：key 放在头里，查询照各家的规矩写进正文或者地址。
#[test]
fn the_rest_requests_carry_the_key_where_each_source_wants_it() {
    let deepseek = rest(
        Kind::Deepseek,
        "https://api.deepseek.com/v1/tools/search",
        &"字".repeat(200),
        80,
        Some("sk-x"),
    )
    .unwrap();
    assert!(!deepseek.get);
    assert_eq!(
        deepseek.headers,
        [("Authorization", "Bearer sk-x".to_string())]
    );
    let body = deepseek.body.unwrap();
    assert_eq!(
        body["query"].as_str().unwrap().chars().count(),
        128,
        "查询至多 128 个字"
    );
    assert_eq!(body["count"], 50, "至多 50 条");

    let tavily = rest(
        Kind::Tavily,
        "https://api.tavily.com/search",
        "rust",
        30,
        Some("tvly-x"),
    )
    .unwrap();
    assert_eq!(
        tavily.headers,
        [("Authorization", "Bearer tvly-x".to_string())]
    );
    assert_eq!(tavily.body.unwrap()["max_results"], 20);

    let brave = rest(
        Kind::Brave,
        "https://api.search.brave.com/res/v1/web/search",
        "rust lang",
        8,
        Some("b-x"),
    )
    .unwrap();
    assert!(brave.get && brave.body.is_none());
    assert_eq!(
        brave.url,
        "https://api.search.brave.com/res/v1/web/search?q=rust+lang&count=8"
    );
    assert!(
        brave
            .headers
            .contains(&("X-Subscription-Token", "b-x".to_string()))
    );

    let searxng = rest(Kind::Searxng, "http://127.0.0.1:8888/", "rust", 8, None).unwrap();
    assert_eq!(
        searxng.url,
        "http://127.0.0.1:8888/search?q=rust&format=json"
    );
    assert!(searxng.headers.is_empty());

    assert!(
        rest(Kind::Exa, "https://mcp.exa.ai/mcp", "rust", 8, None).is_none(),
        "走 MCP 的不走这里"
    );
}
