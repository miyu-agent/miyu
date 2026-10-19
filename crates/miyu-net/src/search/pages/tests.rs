//! 三家代抓的回应怎么读（施工 N-4，照 2026-10-19 实测的样子）：Exa 去掉开头几行，Firecrawl、Parallel 读 JSON；Parallel
//! 没结果的照 `errors` 说；读不懂的照原话认限额。各家的参数照实测写。

use super::*;

#[test]
fn exa_drops_its_header_lines() {
    let answer = "# Hello GPT-4o | OpenAI\nURL: https://openai.com/index/hello-gpt-4o/\nPublished: 2024-05-13\n\nHello GPT-4o\n\nNote: this line stays.";
    let (title, text) = read(Kind::Exa, answer).unwrap();
    assert_eq!(title.as_deref(), Some("Hello GPT-4o | OpenAI"));
    assert_eq!(
        text.trim(),
        "Hello GPT-4o\n\nNote: this line stays.",
        "正文里像开头那样的行留着"
    );
    let (title, text) = read(Kind::Exa, "plain body").unwrap();
    assert_eq!(
        (title, text.as_str()),
        (None, "plain body"),
        "没有标题行的照原样"
    );
}

#[test]
fn firecrawl_and_parallel_read_json() {
    let firecrawl =
        r##"{"markdown": "# Title\n\nBody", "metadata": {"title": "Page", "statusCode": 200}}"##;
    assert_eq!(
        read(Kind::Firecrawl, firecrawl).unwrap(),
        (Some("Page".to_string()), "# Title\n\nBody".to_string())
    );
    let full = r#"{"results": [{"url": "u", "title": "T", "full_content": "Full"}], "errors": []}"#;
    assert_eq!(
        read(Kind::Parallel, full).unwrap(),
        (Some("T".to_string()), "Full".to_string())
    );
    let excerpts = r#"{"results": [{"url": "u", "title": null, "excerpts": ["a", "b"]}]}"#;
    assert_eq!(
        read(Kind::Parallel, excerpts).unwrap(),
        (None, "a\n\nb".to_string())
    );
    let missing = r#"{"results": [], "errors": [{"url": "u", "error_type": "http_error", "http_status_code": 404}]}"#;
    assert_eq!(
        read(Kind::Parallel, missing),
        Err(Failure::Failed("http_error".to_string()))
    );
    assert_eq!(
        read(Kind::Firecrawl, "Rate limit exceeded"),
        Err(Failure::Limited),
        "读不懂的照原话认限额"
    );
    assert!(read(Kind::Bing, "x").is_err(), "抓不了页面的那几种");
}

#[test]
fn each_source_is_asked_with_its_own_tool() {
    let (tool, args) = call(Kind::Exa, "https://a.example/", 5000, "s").unwrap();
    assert_eq!(tool, "web_fetch_exa");
    assert_eq!(
        args,
        json!({"urls": ["https://a.example/"], "maxCharacters": 5000})
    );
    let (tool, args) = call(Kind::Parallel, "https://a.example/", 5000, "s").unwrap();
    assert_eq!(tool, "web_fetch");
    assert_eq!(
        args,
        json!({"urls": ["https://a.example/"], "full_content": true, "session_id": "s"}),
        "要全文，免费额度照这个核心的编号算"
    );
    let (tool, args) = call(Kind::Firecrawl, "https://a.example/", 5000, "s").unwrap();
    assert_eq!(tool, "firecrawl_scrape");
    assert_eq!(args["formats"], json!(["markdown"]));
    assert!(call(Kind::You, "https://a.example/", 5000, "s").is_none());
}
