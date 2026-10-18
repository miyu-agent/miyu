//! 收拾结果、写查询、认限额（施工 N-2 上）。

use super::rules::{Kind, Needs, Source};
use super::{Failure, Hit, Query, clip, domain_of, phrase, tidy};

fn hit(url: &str, snippet: &str) -> Hit {
    Hit {
        title: url.to_string(),
        url: url.to_string(),
        snippet: Some(snippet.to_string()),
        date: None,
    }
}

fn source(site_operator: bool) -> Source {
    Source {
        name: "Test".to_string(),
        kind: Kind::Firecrawl,
        url: "https://example.invalid/mcp".to_string(),
        needs: Needs::Nothing,
        site_operator,
    }
}

#[test]
fn hits_are_deduplicated_filtered_by_domain_clipped_and_capped() {
    let hits = vec![
        hit("https://docs.rs/tokio", "abcdef"),
        hit("https://docs.rs/tokio", "again"),
        hit("https://www.rust-lang.org/learn", "x"),
        hit("https://blog.rust-lang.org/", "y"),
        hit("https://evil.example.com/", "z"),
        hit("", "no url"),
    ];
    let any = Query {
        text: "q".into(),
        ..Query::default()
    };
    let all = tidy(hits.clone(), &any, 10, 3);
    assert_eq!(
        all.iter().map(|hit| hit.url.as_str()).collect::<Vec<_>>(),
        [
            "https://docs.rs/tokio",
            "https://www.rust-lang.org/learn",
            "https://blog.rust-lang.org/",
            "https://evil.example.com/"
        ]
    );
    assert_eq!(all[0].snippet.as_deref(), Some("abc…"), "摘录截到 3 个字");
    assert_eq!(tidy(hits.clone(), &any, 2, 3).len(), 2, "至多 2 条");

    let rust_only = Query {
        text: "q".into(),
        allowed: vec!["https://www.Rust-Lang.org/".into()],
        ..Query::default()
    };
    assert_eq!(
        tidy(hits.clone(), &rust_only, 10, 99)
            .iter()
            .map(|hit| hit.url.as_str())
            .collect::<Vec<_>>(),
        [
            "https://www.rust-lang.org/learn",
            "https://blog.rust-lang.org/"
        ],
        "只要的域名含子域名"
    );
    let no_example = Query {
        text: "q".into(),
        blocked: vec!["example.com".into()],
        ..Query::default()
    };
    assert!(
        tidy(hits, &no_example, 10, 99)
            .iter()
            .all(|hit| !hit.url.contains("example.com"))
    );
}

#[test]
fn site_operators_go_into_the_query_only_for_sources_that_read_them() {
    let query = Query {
        text: " tokio select ".into(),
        allowed: vec!["docs.rs".into(), "*.tokio.rs".into()],
        ..Query::default()
    };
    assert_eq!(
        phrase(&query, &source(true)),
        "tokio select site:docs.rs OR site:tokio.rs"
    );
    assert_eq!(phrase(&query, &source(false)), "tokio select");
    assert_eq!(
        phrase(
            &Query {
                text: "a".into(),
                ..Query::default()
            },
            &source(true)
        ),
        "a"
    );
}

#[test]
fn domains_are_written_as_bare_hosts_and_clipping_respects_characters() {
    assert_eq!(domain_of("  HTTPS://www.Example.com/path "), "example.com");
    assert_eq!(clip("一二三四", 2), "一二…");
    assert_eq!(clip("一二", 2), "一二");
}

#[test]
fn quota_words_and_429_mean_limited() {
    for text in [
        "Rate limit exceeded",
        "Too Many Requests",
        "Anonymous quota exhausted",
        "out of credits",
        "HTTP 429: slow down",
    ] {
        assert_eq!(Failure::from_text(text), Failure::Limited, "{text}");
    }
    assert_eq!(
        Failure::from_text("bad query"),
        Failure::Failed("bad query".into())
    );
}
