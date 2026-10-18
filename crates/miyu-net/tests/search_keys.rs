//! 要 key、要地址的几家（施工 N-3 上，设计 36 第二节）：key 带在各家要的地方；没配的跳过；DeepSeek 在没配置时的顺序里，
//! 有 key 才用；SearXNG 照配的地址；Exa 配了 key 的走自己的额度（接在地址后面）。假服务器在回环上，几家的地址照
//! `Search::testing` 换成它，`Using` 照手写的。

use std::time::Duration;

use serde_json::json;

use miyu_net::{Found, Missed, Query, Search, Using};

use crate::support::{Reply, Site, resources};

const SOURCES: [&str; 8] = [
    "exa",
    "parallel",
    "firecrawl",
    "anysearch",
    "you",
    "deepseek",
    "tavily",
    "brave",
];

/// 几家都指到这台假服务器：`/exa`、`/deepseek`……
fn search(site: &Site) -> Search {
    let urls = SOURCES
        .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
        .to_vec();
    Search::new(&resources()).testing(urls)
}

async fn ask(search: &Search, using: &Using) -> Result<Found, Missed> {
    let query = Query {
        text: "rust".to_string(),
        ..Query::default()
    };
    tokio::time::timeout(Duration::from_secs(60), search.search(&query, using))
        .await
        .expect("一分钟内搜完")
}

fn results(results: serde_json::Value) -> Reply {
    Reply::bytes("application/json", results.to_string().as_bytes())
}

/// 一份 MCP 的工具结果（Exa 的文字写法）。
fn exa() -> Reply {
    let text = "Title: Exa hit\\nURL: https://exa.example/\\nPublished: N/A\\nAuthor: N/A\\nHighlights:\\nfrom exa\\n";
    let body =
        json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": text}]}});
    Reply::bytes("application/json", body.to_string().as_bytes())
}

/// 出厂的五家免 key 的都 500：顺序走到最后。
fn free_ones_fail() -> Vec<(String, Reply)> {
    ["exa", "parallel", "firecrawl", "anysearch", "you"]
        .map(|id| (format!("/{id}"), Reply::status(500)))
        .to_vec()
}

#[tokio::test]
async fn a_picked_keyed_source_carries_its_key_and_is_skipped_without_one() {
    let site = Site::start(vec![
        ("/tavily".to_string(), results(json!({"results": [{"title": "T", "url": "https://t.example/", "content": "c"}]}))),
        ("/brave*".to_string(), results(json!({"web": {"results": [{"title": "B", "url": "https://b.example/", "description": "d"}]}}))),
        ("/exa".to_string(), exa()),
    ])
    .await;
    let search = search(&site);

    let tavily = ask(
        &search,
        &Using::picked("tavily").with_key("tavily", "tvly-1"),
    )
    .await
    .expect("搜成了");
    assert_eq!(tavily.source, "Tavily");
    let seen = site.seen();
    let asked = seen
        .iter()
        .find(|seen| seen.target == "/tavily")
        .expect("问了 Tavily");
    assert_eq!(
        asked.authorization, "Bearer tvly-1",
        "key 放在 Authorization 里"
    );

    let brave = ask(&search, &Using::picked("brave").with_key("brave", "b-1"))
        .await
        .expect("搜成了");
    assert_eq!(brave.source, "Brave");
    let seen = site.seen();
    let asked = seen
        .iter()
        .find(|seen| seen.target.starts_with("/brave?"))
        .expect("问了 Brave");
    assert_eq!(asked.token, "b-1", "key 放在 X-Subscription-Token 里");
    assert!(asked.target.contains("q=rust"), "{}", asked.target);

    let skipped = ask(&search, &Using::picked("tavily"))
        .await
        .expect("搜成了");
    assert_eq!(skipped.source, "Exa", "没配 key 的跳过，照出厂的顺序");
    assert_eq!(site.hits("/tavily"), 1, "没配 key 的那一次没去问");
}

#[tokio::test]
async fn deepseek_closes_the_shipped_order_only_with_a_key() {
    let mut routes = free_ones_fail();
    routes.push((
        "/deepseek".to_string(),
        results(json!({"results": [{"title": "D", "url": "https://d.example/", "summary": "s", "date": "2026-10-01T00:00:00Z"}]})),
    ));
    let site = Site::start(routes).await;
    let search = search(&site);

    let without = ask(&search, &Using::default()).await;
    assert!(matches!(without, Err(Missed::Failed(_))), "{without:?}");
    assert_eq!(site.hits("/deepseek"), 0, "没 key 不问 DeepSeek");

    let with = ask(&search, &Using::default().with_key("deepseek", "sk-1"))
        .await
        .expect("搜成了");
    assert_eq!(
        with.source, "DeepSeek",
        "免 key 的都不成，有 key 的 DeepSeek 兜在后面"
    );
    assert_eq!(with.hits[0].date.as_deref(), Some("2026-10-01"));
    let seen = site.seen();
    assert_eq!(
        seen.iter()
            .find(|seen| seen.target == "/deepseek")
            .expect("问了")
            .authorization,
        "Bearer sk-1"
    );
}

#[tokio::test]
async fn searxng_is_asked_at_the_configured_address_and_exa_carries_its_own_key() {
    let site = Site::start(vec![
        (
            "/my-searx/search*".to_string(),
            results(
                json!({"results": [{"title": "S", "url": "https://s.example/", "content": "c"}]}),
            ),
        ),
        ("/exa*".to_string(), exa()),
    ])
    .await;
    let search = search(&site);

    let searx = ask(
        &search,
        &Using::picked("searxng").with_address(&site.url("/my-searx/")),
    )
    .await
    .expect("搜成了");
    assert_eq!(searx.source, "SearXNG");
    assert!(
        site.seen()
            .iter()
            .any(|seen| seen.target.starts_with("/my-searx/search?")
                && seen.target.contains("format=json")),
        "{:?}",
        site.seen()
    );
    let no_address = ask(&search, &Using::picked("searxng"))
        .await
        .expect("搜成了");
    assert_eq!(no_address.source, "Exa", "没配地址的跳过");

    let keyed = ask(&search, &Using::default().with_key("exa", "exa-1"))
        .await
        .expect("搜成了");
    assert_eq!(keyed.source, "Exa");
    assert!(
        site.seen().iter().any(|seen| seen.target.starts_with("/exa?") && seen.target.contains("exaApiKey=exa-1")),
        "配了 key 的 Exa 接在地址后面：{:?}",
        site.seen()
    );
}

/// 余额不足（HTTP 402，DeepSeek 的写法）当限额：进冷却，下一次不问它（施工 N-3 上）。
#[tokio::test]
async fn running_out_of_balance_counts_as_a_limit() {
    let site = Site::start(vec![
        (
            "/deepseek".to_string(),
            // 官方的出错是 OpenAI 那种 JSON：只看正文会当成「没搜到」，得照状态码认。
            Reply::bytes(
                "application/json",
                br#"{"error":{"message":"Insufficient Balance","type":"unknown_error","param":null,"code":"invalid_request_error"}}"#,
            )
            .with_status(402),
        ),
        ("/exa".to_string(), exa()),
    ])
    .await;
    let search = search(&site);
    let using = Using::picked("deepseek").with_key("deepseek", "sk-1");

    let first = ask(&search, &using).await.expect("Exa 兜住了");
    assert_eq!(first.source, "Exa");
    assert_eq!(site.hits("/deepseek"), 1);
    assert_eq!(
        search
            .cooling()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        ["deepseek"],
        "进了冷却"
    );

    ask(&search, &using).await.expect("Exa 兜住了");
    assert_eq!(site.hits("/deepseek"), 1, "冷却着，不再问");
}
