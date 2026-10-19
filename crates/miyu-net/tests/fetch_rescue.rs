//! 自己抓不下来的请别家代抓（施工 N-4，设计 36 的 N-4）：本机的假服务器上整条走通。被挡（403）、抓回来短得像登录墙的，照
//! Exa → Firecrawl → Parallel 试，正文太短的换下一家；代抓到的给她正文、给人写是哪一家，翻页不再去抓；设置里挑了能抓页面的
//! 那一家的排在最前；都不成的照自己抓的原因说；404 这种不代抓。几家的地址照 `Search::testing` 换成这台假服务器。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::Value as ConfigValue;
use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_kernel::block::Block;
use miyu_net::{Pages, Search, WEB_FETCH};
use miyu_tool::{Call, Done, SettingsPort, Tool};

use crate::support::{Reply, Site, local, resources};

/// 一份 MCP 的工具结果：文字是 `text`。握手也回它，客户端只看状态码。
fn mcp(text: &str) -> Reply {
    let body =
        json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": text}]}});
    Reply::bytes("application/json", body.to_string().as_bytes())
}

/// 一段够长的正文。
fn body(word: &str) -> String {
    format!("{word} ").repeat(80)
}

/// 三家都指到这台假服务器：`/exa`、`/parallel`、`/firecrawl`。
fn tool(site: &Site) -> Arc<dyn Tool> {
    let urls = ["exa", "parallel", "firecrawl"]
        .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
        .to_vec();
    let pages = Pages::new(&resources()).testing(local());
    let search = Search::new(&resources()).testing(urls);
    miyu_net::tools_with(&resources(), Arc::new(pages), Arc::new(search))
        .expect("字读得出")
        .into_iter()
        .find(|tool| tool.spec().name == WEB_FETCH)
        .expect("有 web_fetch")
}

/// 设置里挑了 `source` 的那一轮的配置。
struct Picked(&'static str);

impl SettingsPort for Picked {
    fn values(&self) -> Values {
        let mut values = Values::defaults(miyu_net::SearchSettings::ITEMS);
        values.set("net.search.source", ConfigValue::Text(self.0.into()));
        values
    }

    fn secret(&self, _reference: &Reference) -> Option<Secret> {
        None
    }
}

async fn fetch(tool: &Arc<dyn Tool>, args: Value, settings: Option<Arc<dyn SettingsPort>>) -> Done {
    let call = Call {
        settings,
        ..crate::fetch::call(args)
    };
    crate::fetch::run(tool, call).await
}

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 给人看的那一句：编号去掉 `software/net/`，`via` 那一格。
fn said(done: &Done) -> (String, Option<String>) {
    let human = done.human.as_ref().expect("有给人看的一句");
    (
        human.key.trim_start_matches("software/net/").to_string(),
        human.fields.get("via").cloned(),
    )
}

#[tokio::test]
async fn a_blocked_page_is_fetched_by_the_first_source_that_has_it() {
    let exa = format!("# Hello\nURL: x\nPublished: 2026-10-19\n\n{}", body("exa"));
    let site = Site::start(vec![
        ("/page".to_string(), Reply::status(403)),
        ("/exa".to_string(), mcp(&exa)),
    ])
    .await;
    let tool = tool(&site);
    let url = site.url("/page");
    let done = fetch(&tool, json!({"url": url}), None).await;
    assert!(!done.error, "{}", text(&done));
    assert!(text(&done).starts_with("exa exa"), "{}", text(&done));
    assert_eq!(
        said(&done),
        ("web_fetch/titled".to_string(), Some("Exa".to_string())),
        "给人写是哪一家"
    );
    assert_eq!(site.hits("/firecrawl"), 0, "第一家成了不问后面的");
    let again = fetch(&tool, json!({"url": url}), None).await;
    assert!(!again.error);
    assert_eq!(site.hits("/page"), 1, "代抓到的记着，不再自己抓");
    let asked = site.hits("/exa");
    fetch(&tool, json!({"url": url}), None).await;
    assert_eq!(site.hits("/exa"), asked, "也不再请别家");
}

#[tokio::test]
async fn too_little_text_moves_on_to_the_next_source() {
    let firecrawl =
        json!({"markdown": "请您登录后查看", "metadata": {"title": "Login"}}).to_string();
    let parallel =
        json!({"results": [{"url": "u", "title": "Real", "full_content": body("parallel")}]})
            .to_string();
    let site = Site::start(vec![
        (
            "/page".to_string(),
            Reply::html("<html><body><p>Loading</p></body></html>"),
        ),
        (
            "/exa".to_string(),
            mcp("# Redirect\n\nYour browser should have redirected you."),
        ),
        ("/firecrawl".to_string(), mcp(&firecrawl)),
        ("/parallel".to_string(), mcp(&parallel)),
    ])
    .await;
    let tool = tool(&site);
    let done = fetch(&tool, json!({"url": site.url("/page")}), None).await;
    assert!(text(&done).starts_with("parallel"), "{}", text(&done));
    assert_eq!(said(&done).1.as_deref(), Some("Parallel"));
    assert_eq!(
        (site.hits("/exa") > 0, site.hits("/firecrawl") > 0),
        (true, true),
        "短得像登录墙的自己抓的、Exa、Firecrawl 都试过"
    );
}

#[tokio::test]
async fn a_picked_source_goes_first_and_failures_keep_her_own_reason() {
    let site = Site::start(vec![
        ("/page".to_string(), Reply::status(403)),
        (
            "/firecrawl".to_string(),
            mcp(&json!({"markdown": body("fc")}).to_string()),
        ),
    ])
    .await;
    let tool = tool(&site);
    let picked: Arc<dyn SettingsPort> = Arc::new(Picked("firecrawl"));
    let done = fetch(&tool, json!({"url": site.url("/page")}), Some(picked)).await;
    assert_eq!(said(&done).1.as_deref(), Some("Firecrawl"));
    assert_eq!(site.hits("/exa"), 0, "挑了的那一家排在最前");

    let failing = Site::start(vec![("/page".to_string(), Reply::status(403))]).await;
    let tool = self::tool(&failing);
    let done = fetch(&tool, json!({"url": failing.url("/page")}), None).await;
    assert!(done.error);
    assert_eq!(
        said(&done),
        ("web_fetch/status".to_string(), None),
        "都不成的照自己的原因"
    );
}

#[tokio::test]
async fn a_missing_page_is_not_fetched_by_others() {
    let site = Site::start(vec![("/page".to_string(), Reply::status(404))]).await;
    let tool = tool(&site);
    let done = fetch(&tool, json!({"url": site.url("/page")}), None).await;
    assert!(done.error);
    assert_eq!(
        site.hits("/exa") + site.hits("/firecrawl") + site.hits("/parallel"),
        0,
        "404 不请别家"
    );
}

#[tokio::test]
async fn an_empty_page_is_fetched_by_others_too() {
    let site = Site::start(vec![
        (
            "/page".to_string(),
            Reply::html("<html><body></body></html>"),
        ),
        ("/exa".to_string(), mcp(&body("exa"))),
    ])
    .await;
    let tool = tool(&site);
    let done = fetch(&tool, json!({"url": site.url("/page")}), None).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        said(&done),
        ("web_fetch/fetched".to_string(), Some("Exa".to_string()))
    );
}
