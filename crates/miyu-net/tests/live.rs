//! 真网络的实测（施工单 W-7 再补「验收」第 3 条）：照出厂的规矩、平时的闸和环境变量里的代理，去抓真的站，把卡片
//! 印出来给人看。要连外网，结果跟着对方的站变，所以两道锁：标了 `#[ignore]`，平时的 `cargo test` 不跑；CI 的长跑
//! 那一项用 `--ignored` 跑所有标了的测试，所以还要设 `MIYU_NET_LIVE=1` 才真去连，没设的什么都不做。在能上网的
//! 机器上这样跑：
//!
//! ```text
//! MIYU_NET_LIVE=1 cargo test -p miyu-net --test live -- --ignored --nocapture
//! ```
//!
//! 图存进一个用完就删的临时目录，不碰真实数据。

use std::time::Duration;

use crate::support::{Store, resources};
use miyu_net::{Kind, LinkPreview, Preview};

/// 要试的链接和应该是什么：（链接，卡片的 `kind`）。
const LINKS: &[(&str, Kind)] = &[
    ("https://www.bilibili.com/video/BV1Rxam6kEtU", Kind::Video),
    ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", Kind::Video),
    (
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        Kind::Article,
    ),
    ("https://zh.wikipedia.org/wiki/Rust", Kind::Article),
    ("https://wiki.archlinux.org/title/Pacman", Kind::Article),
    ("https://github.com/rust-lang/rust", Kind::Page),
];

#[tokio::test]
#[ignore = "要连外网：设 MIYU_NET_LIVE=1 在能上网的机器上手动跑"]
async fn real_sites_make_real_cards() {
    if std::env::var_os("MIYU_NET_LIVE").is_none() {
        println!("没设 MIYU_NET_LIVE=1：不连外网，跳过");
        return;
    }
    let store = Store::new();
    let links = LinkPreview::new(&resources(), store.blobs.clone());
    let mut wrong = Vec::new();
    for (url, kind) in LINKS {
        let got = tokio::time::timeout(Duration::from_secs(60), links.preview(url))
            .await
            .unwrap_or_else(|_| panic!("一分钟内没抓完 {url}"))
            .expect("link_preview.json 读得懂");
        println!("\n== {url}");
        match got {
            Preview::Card(card) => {
                println!("  kind:        {}", card.kind.as_str());
                println!("  title:       {}", card.title);
                println!("  description: {}", card.description);
                println!("  site:        {}", card.site);
                println!("  author:      {:?}", card.author);
                println!("  duration:    {:?}", card.duration);
                println!(
                    "  image:       {:?}",
                    card.image.map(|image| image.media_type)
                );
                println!("  icon:        {:?}", card.icon.map(|icon| icon.media_type));
                println!("  url:         {}", card.url);
                if card.kind != *kind || card.description.is_empty() {
                    wrong.push(format!(
                        "{url}：kind 是 {:?}，简介 {:?}",
                        card.kind, card.description
                    ));
                }
            }
            Preview::Miss(why) => {
                println!("  没有卡片：{}", why.as_str());
                wrong.push(format!("{url}：没有卡片（{}）", why.as_str()));
            }
        }
    }
    assert!(wrong.is_empty(), "这几条不像样：\n{}", wrong.join("\n"));
}

/// 要抓的网页和读出来应该有的字（施工 N-1）：网页、维基、接口回的 JSON、纯文字。
const PAGES: &[(&str, &str)] = &[
    (
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        "Rust",
    ),
    ("https://zh.wikipedia.org/wiki/Rust", "Rust"),
    ("https://wiki.archlinux.org/title/Pacman", "pacman"),
    (
        "https://api.github.com/repos/rust-lang/rust",
        "\"full_name\"",
    ),
    (
        "https://raw.githubusercontent.com/rust-lang/rust/master/README.md",
        "Rust",
    ),
    ("https://www.rust-lang.org/", "Rust"),
];

#[tokio::test]
#[ignore = "要连外网：设 MIYU_NET_LIVE=1 在能上网的机器上手动跑"]
async fn real_pages_are_read_by_web_fetch() {
    if std::env::var_os("MIYU_NET_LIVE").is_none() {
        println!("没设 MIYU_NET_LIVE=1：不连外网，跳过");
        return;
    }
    let tool = miyu_net::tools(&resources())
        .expect("字读得出")
        .into_iter()
        .find(|tool| tool.spec().name == miyu_net::WEB_FETCH)
        .expect("有 web_fetch");
    let mut wrong = Vec::new();
    for (url, word) in PAGES {
        let done = crate::fetch::fetch(&tool, serde_json::json!({"url": url})).await;
        let read = crate::fetch::text(&done);
        let head: String = read.chars().take(400).collect();
        let tail: String = read
            .chars()
            .rev()
            .take(60)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        println!(
            "\n== {url}（{} 个字）\n{head}\n……{tail}",
            read.chars().count()
        );
        if done.error || !read.contains(word) {
            wrong.push(format!(
                "{url}：{}",
                read.lines().next().unwrap_or_default()
            ));
        }
    }
    assert!(wrong.is_empty(), "这几条没读成：\n{}", wrong.join("\n"));
}

/// 一家家试的查询和该搜到的字（施工 N-2 上）。
const QUERIES: &[(&str, &str)] = &[
    ("tokio select macro", "tokio"),
    ("Rust 程序设计语言 官网", "rust"),
];

#[tokio::test]
#[ignore = "要连外网：设 MIYU_NET_LIVE=1 在能上网的机器上手动跑"]
async fn every_keyless_source_answers_web_search() {
    if std::env::var_os("MIYU_NET_LIVE").is_none() {
        println!("没设 MIYU_NET_LIVE=1：不连外网，跳过");
        return;
    }
    let mut wrong = Vec::new();
    for id in ["exa", "parallel", "firecrawl", "anysearch", "you"] {
        let search = miyu_net::Search::new(&resources()).only(&[id]);
        for (query, word) in QUERIES {
            let query = miyu_net::Query {
                text: (*query).to_string(),
                ..miyu_net::Query::default()
            };
            let started = std::time::Instant::now();
            match tokio::time::timeout(Duration::from_secs(60), search.search(&query, None)).await {
                Ok(Ok(found)) => {
                    println!(
                        "\n== {id}「{}」{} 条，{:.1} 秒",
                        query.text,
                        found.hits.len(),
                        started.elapsed().as_secs_f32()
                    );
                    for hit in found.hits.iter().take(3) {
                        let snippet: String = hit
                            .snippet
                            .as_deref()
                            .unwrap_or("")
                            .chars()
                            .take(120)
                            .collect();
                        println!(
                            "  {} | {} | {:?}\n    {}",
                            hit.title,
                            hit.url,
                            hit.date,
                            snippet.replace('\n', " ")
                        );
                    }
                    let hay = found
                        .hits
                        .iter()
                        .map(|hit| format!("{} {}", hit.title, hit.url))
                        .collect::<String>()
                        .to_lowercase();
                    if found.hits.is_empty() || !hay.contains(word) {
                        wrong.push(format!(
                            "{id}「{}」：{} 条，没有 {word}",
                            query.text,
                            found.hits.len()
                        ));
                    }
                }
                Ok(Err(missed)) => {
                    println!("\n== {id}「{}」没成：{missed:?}", query.text);
                    wrong.push(format!("{id}「{}」：{missed:?}", query.text));
                }
                Err(_) => wrong.push(format!("{id}「{}」：一分钟没搜完", query.text)),
            }
        }
    }
    assert!(wrong.is_empty(), "这几条没搜成：\n{}", wrong.join("\n"));
}
