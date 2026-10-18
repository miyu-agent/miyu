//! 各家交回的样子读成一样的几条（样本照 2026-10-18 实测的回应删短）。

use super::read;
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
