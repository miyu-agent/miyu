//! 别人要她抓网页（施工 MB-2，设计 06 U25 第 2 条「成员上网不碰本机和局域网」，项目主人 2026-10-22 定）：本机、局域网的地址
//! 一律不去（这一道闸对谁都一样）；系统配置里的内部域名（`net.internal_domains`）别人一律不去，子域名、解析成公网的、跳过去的
//! 都算；配了代理时本机解析不出来的，一个词的、带内网后缀的别人不交给代理，别的照旧交。机器主人照旧。是不是机器主人照调用的
//! 事实（`Call::caller`），没有的照最严。别人被拒的说「过不了闸」，不请别家代抓（内部的网址不发给第三方）。

use std::sync::Arc;

use miyu_config::secret::{Reference, Secret};
use miyu_config::{Value as ConfigValue, Values};
use miyu_kernel::id::VenueId;
use miyu_net::testkit::Testing;
use miyu_tool::{Call, Caller, SettingsPort};
use serde_json::json;

use crate::fetch::{call, run, said, text, tool};
use crate::support::{Reply, Site, ip, page};

/// 系统配置里写了这几个内部域名的那一轮。
struct Internal(Vec<&'static str>);

impl SettingsPort for Internal {
    fn values(&self) -> Values {
        let mut values = Values::default();
        let domains = self
            .0
            .iter()
            .map(|domain| ConfigValue::Text((*domain).to_string().into()))
            .collect();
        values.set("net.internal_domains", ConfigValue::List(domains));
        values
    }

    fn secret(&self, _reference: &Reference) -> Option<Secret> {
        None
    }
}

/// 本机会话里的一次调用，开口的是不是机器主人照 `owner`，内部域名照 `internal`。
fn asked(url: &str, owner: bool, internal: Vec<&'static str>) -> Call {
    Call {
        caller: Some(Caller {
            venue: VenueId::parse("local").expect("合写法"),
            owner,
            model: None,
            log: None,
        }),
        settings: Some(Arc::new(Internal(internal))),
        ..call(json!({ "url": url }))
    }
}

/// 代理照 `proxy` 这台假代理；`example-corp.test` 和它的子域、`public.test` 解析到测性能的段（当公网，从本机连不上，只有经
/// 代理才到得了），别的名字解析不出来。
fn proxied(proxy: &Site) -> Testing {
    Testing {
        loopback_public: false,
        hosts: vec![
            ("example-corp.test".to_string(), ip("198.18.0.1")),
            ("wiki.example-corp.test".to_string(), ip("198.18.0.2")),
            ("notexample-corp.test".to_string(), ip("198.18.0.3")),
            ("public.test".to_string(), ip("198.18.0.4")),
        ],
        proxy: Some(format!("http://127.0.0.1:{}", proxy.port)),
        no_proxy: String::new(),
    }
}

/// 假代理什么都回一页。
async fn answering(urls: &[&str]) -> Site {
    Site::start(
        urls.iter()
            .map(|url| ((*url).to_string(), Reply::html(&page("Got it", ""))))
            .collect(),
    )
    .await
}

#[tokio::test]
async fn others_reach_neither_the_machine_nor_the_lan() {
    let site = Site::start(vec![("/".to_string(), Reply::html(&page("Here", "")))]).await;
    let tool = tool(Testing {
        loopback_public: false,
        hosts: Vec::new(),
        proxy: None,
        no_proxy: String::new(),
    });
    for url in [
        site.url("/"),
        "http://192.168.1.1/".to_string(),
        "http://10.0.0.2/".to_string(),
        "http://printer.local/".to_string(),
        "http://localhost/".to_string(),
    ] {
        let done = run(&tool, asked(&url, false, Vec::new())).await;
        assert_eq!(said(&done), "web_fetch/blocked", "{url}: {}", text(&done));
    }
}

#[tokio::test]
async fn an_internal_looking_name_unknown_here_is_not_handed_to_the_proxy_for_others() {
    let inside = [
        "http://nas/",
        "http://gitlab.corp/",
        "http://wiki.home.arpa/",
        "http://box.lan/",
        "http://hr.intranet/",
    ];
    let proxy = answering(&[&inside[..], &["http://blocked.example/"][..]].concat()).await;
    let tool = tool(proxied(&proxy));
    for url in inside {
        let other = run(&tool, asked(url, false, Vec::new())).await;
        assert_eq!(said(&other), "web_fetch/blocked", "{url}: {}", text(&other));
        assert_eq!(proxy.hits(url), 0, "{url} 没交给代理");
    }
    // 被墙的境外站常这样：本机解析不出来、代理那头解析得出来。别人照样抓得到。
    let outside = run(&tool, asked("http://blocked.example/", false, Vec::new())).await;
    assert!(!outside.error, "{}", text(&outside));
    assert_eq!(proxy.hits("http://blocked.example/"), 1);
    // 机器主人照旧交给代理。
    let owner = run(&tool, asked("http://nas/", true, Vec::new())).await;
    assert!(!owner.error, "{}", text(&owner));
    assert_eq!(proxy.hits("http://nas/"), 1);
    // 机器主人抓到的记着，别人再要照旧不给；拿不到调用的事实的照最严。
    let again = run(&tool, asked("http://nas/", false, Vec::new())).await;
    assert_eq!(said(&again), "web_fetch/blocked", "{}", text(&again));
    let unknown = run(&tool, call(json!({ "url": "http://gitlab.corp/" }))).await;
    assert_eq!(said(&unknown), "web_fetch/blocked", "{}", text(&unknown));
    assert_eq!(proxy.hits("http://gitlab.corp/"), 0);
}

#[tokio::test]
async fn listed_domains_are_off_limits_to_others_however_they_resolve() {
    let proxy = Site::start(vec![
        (
            "http://example-corp.test/".to_string(),
            Reply::html(&page("Corp", "")),
        ),
        (
            "http://wiki.example-corp.test/".to_string(),
            Reply::html(&page("Wiki", "")),
        ),
        (
            "http://notexample-corp.test/".to_string(),
            Reply::html(&page("Someone else", "")),
        ),
        (
            "http://public.test/go".to_string(),
            Reply::redirect("http://wiki.example-corp.test/"),
        ),
    ])
    .await;
    let tool = tool(proxied(&proxy));
    let listed = vec![".Example-Corp.TEST"];
    for url in [
        "http://example-corp.test/",
        "http://wiki.example-corp.test/",
        "http://public.test/go",
    ] {
        let other = run(&tool, asked(url, false, listed.clone())).await;
        assert_eq!(said(&other), "web_fetch/blocked", "{url}: {}", text(&other));
    }
    assert_eq!(
        proxy.hits("http://example-corp.test/"),
        0,
        "解析成公网的也不去"
    );
    assert_eq!(
        proxy.hits("http://wiki.example-corp.test/"),
        0,
        "跳过去的也不去"
    );
    // 名字只是长得像的不算。
    let near = run(
        &tool,
        asked("http://notexample-corp.test/", false, listed.clone()),
    )
    .await;
    assert!(!near.error, "{}", text(&near));
    // 机器主人不受清单限制。
    let owner = run(&tool, asked("http://wiki.example-corp.test/", true, listed)).await;
    assert!(!owner.error, "{}", text(&owner));
}
