//! `web_search` 在本机的假服务器上整条走通（施工 N-2 上，设计 36 第二节）：五家照顺序试，限额的进冷却、下一次跳过；出错的
//! 这一次换下一家；成了的写给她几条、给人写是哪一家、给头一份链接列表；都不成的照最后一家的原因说；都冷却着的说一声。
//! 假服务器在回环上：几家的地址照 `Search::testing` 换成它。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_config::Value as ConfigValue;
use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_kernel::block::Block;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_net::{Pages, Search, WEB_SEARCH};
use miyu_tool::{Call, Done, Progress, SettingsPort, Stop, Tool};

use crate::support::{Reply, Site, resources};

/// 一份 MCP 的工具结果：文字是 `text`。握手也回它，客户端只看状态码。
fn mcp(text: &str) -> Reply {
    let body =
        json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": text}]}});
    Reply::bytes("application/json", body.to_string().as_bytes())
}

/// 一份 SSE 写的 MCP 工具结果，出错的。
fn mcp_error_sse(text: &str) -> Reply {
    let body = json!({"jsonrpc": "2.0", "id": 2, "result": {"isError": true, "content": [{"type": "text", "text": text}]}});
    Reply::bytes(
        "text/event-stream",
        format!("event: message\ndata: {body}\n\n").as_bytes(),
    )
}

/// AnySearch 的一份回应。
fn anysearch(results: Value) -> Reply {
    let body = json!({"code": 0, "message": "success", "data": {"results": results}});
    Reply::bytes("application/json", body.to_string().as_bytes())
}

const EXA: &str = "Title: Tokio select\nURL: https://docs.rs/tokio/latest/tokio/macro.select.html\nPublished: 2026-09-01T00:00:00Z\nAuthor: N/A\nHighlights:\nWaits on multiple concurrent branches.\n\nTitle: Spam\nURL: https://spam.example.com/x\nPublished: N/A\nAuthor: N/A\nHighlights:\nbuy now\n";

/// 几家都指到这台假服务器：`/exa`、`/parallel`、`/firecrawl`、`/anysearch`、`/you`。
fn tool(site: &Site) -> Arc<dyn Tool> {
    let urls = [
        "exa",
        "parallel",
        "firecrawl",
        "anysearch",
        "you",
        "duckduckgo",
        "bing",
    ]
    .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
    .to_vec();
    let search = Search::new(&resources()).testing(urls);
    miyu_net::tools_with(
        &resources(),
        Arc::new(Pages::new(&resources())),
        Arc::new(search),
    )
    .expect("字读得出")
    .into_iter()
    .find(|tool| tool.spec().name == WEB_SEARCH)
    .expect("有 web_search")
}

/// 调一次，等最多一分钟。
async fn search(tool: &Arc<dyn Tool>, args: Value) -> Done {
    search_with(tool, args, None).await
}

/// 设置里挑了 `source` 的那一轮的配置（施工 N-2 下）。
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

/// 带着这一轮的配置 `settings` 调一次。
async fn search_with(
    tool: &Arc<dyn Tool>,
    args: Value,
    settings: Option<Arc<dyn SettingsPort>>,
) -> Done {
    let call = Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Arc::default(),
        stop: Stop::default(),
        sandbox: None,
        log: None,
        offset: UtcOffset::from_minutes(0).expect("零时区"),
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: None,
        knowledge: None,
        goals: None,
        nested: None,
        packages: None,
        ids: None,
        settings,
    };
    tokio::time::timeout(
        Duration::from_secs(60),
        tool.run(call, Progress::new(|_| {})),
    )
    .await
    .expect("一分钟内搜完")
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

/// 给人看的说法：编号去掉 `software/net/`，带上 `source` 那一格。
fn said(done: &Done) -> (String, Option<String>) {
    let human = done.human.as_ref().expect("有给人看的一句");
    (
        human.key.trim_start_matches("software/net/").to_string(),
        human.fields.get("via").cloned(),
    )
}

#[tokio::test]
async fn the_tool_is_read_only_and_needs_only_a_query() {
    let site = Site::start(Vec::new()).await;
    let tool = tool(&site);
    assert_eq!(tool.spec().access, Access::Read, "只读：哪里都不问人");
    let parameters: Value = serde_json::from_str(tool.spec().parameters.get()).unwrap();
    assert_eq!(parameters["required"], json!(["query"]));
    let done = search(&tool, json!({"query": "  "})).await;
    assert!(done.error);
    assert_eq!(said(&done).0, "web_search/bad-args");
}

#[tokio::test]
async fn the_first_source_answers_and_the_result_is_written_for_her_people_and_heads() {
    let site = Site::start(vec![("/exa".to_string(), mcp(EXA))]).await;
    let tool = tool(&site);

    let done = search(
        &tool,
        json!({"query": "tokio select", "blocked_domains": ["example.com"]}),
    )
    .await;

    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "[1] Tokio select\nhttps://docs.rs/tokio/latest/tokio/macro.select.html (2026-09-01)\nWaits on multiple concurrent branches.",
        "不要的域名筛掉；结果里不写是哪一家"
    );
    assert_eq!(
        said(&done),
        ("web_search/found/one".to_string(), Some("Exa".to_string())),
        "一条的说单数，写是哪一家"
    );
    assert_eq!(done.data, done.detail, "给脚本的是同一份（施工 M-2）");
    assert_eq!(
        done.detail,
        Some(json!({"kind": "links", "links": [{
            "title": "Tokio select", "url": "https://docs.rs/tokio/latest/tokio/macro.select.html",
            "snippet": "Waits on multiple concurrent branches.", "site": "docs.rs", "date": "2026-09-01"}]})),
        "给头的是通用形状的链接列表"
    );
    assert_eq!(site.hits("/parallel"), 0, "第一家成了不问后面的");
}

#[tokio::test]
async fn limited_sources_cool_down_and_failing_ones_are_passed_over() {
    let site = Site::start(vec![
        ("/exa".to_string(), Reply::status(429)),
        (
            "/parallel".to_string(),
            mcp_error_sse("Free tier rate limit exceeded"),
        ),
        ("/firecrawl".to_string(), Reply::status(500)),
        (
            "/anysearch".to_string(),
            anysearch(
                json!([{"title": "One", "url": "https://a.example.org/1", "snippet": "s1"},
                             {"title": "Two", "url": "https://b.example.org/2", "snippet": "s2"}]),
            ),
        ),
    ])
    .await;
    let tool = tool(&site);

    let first = search(&tool, json!({"query": "rust"})).await;
    assert_eq!(
        said(&first),
        (
            "web_search/found".to_string(),
            Some("AnySearch".to_string())
        )
    );
    assert!(
        text(&first).starts_with("[1] One\nhttps://a.example.org/1\ns1\n\n[2] Two"),
        "{}",
        text(&first)
    );
    let (exa, parallel, firecrawl) = (
        site.hits("/exa"),
        site.hits("/parallel"),
        site.hits("/firecrawl"),
    );
    assert!(
        exa >= 1 && parallel >= 1 && firecrawl >= 1,
        "照顺序都试过：{exa} {parallel} {firecrawl}"
    );

    let second = search(&tool, json!({"query": "rust"})).await;
    assert_eq!(said(&second).1.as_deref(), Some("AnySearch"));
    assert_eq!(site.hits("/exa"), exa, "限额的冷却着，跳过");
    assert_eq!(site.hits("/parallel"), parallel, "原话说限额的也冷却");
    assert!(
        site.hits("/firecrawl") > firecrawl,
        "出错的不冷却，下一次照样试"
    );
}

#[tokio::test]
async fn when_every_source_fails_the_last_reason_is_told_and_when_all_cool_that_is_told() {
    // 前面几家都限额，最后一家（网页抓取兜底的 Bing，施工 N-3 下）回 503
    let mut routes: Vec<(String, Reply)> = [
        "exa",
        "parallel",
        "firecrawl",
        "anysearch",
        "you",
        "duckduckgo",
    ]
    .map(|id| (format!("/{id}*"), Reply::status(429)))
    .to_vec();
    routes.push(("/bing*".to_string(), Reply::status(503)));
    let site = Site::start(routes).await;
    let tool = tool(&site);

    let failed = search(&tool, json!({"query": "rust"})).await;
    assert!(failed.error);
    assert_eq!(said(&failed).0, "web_search/failed");
    assert!(
        text(&failed).starts_with("Web search failed: Bing: HTTP 503"),
        "照最后一家的原因说：{}",
        text(&failed)
    );

    // Bing 出错的不冷却：再搜只剩它，它又出错。挑了它的（施工 N-2 下）先问它，出厂顺序里不再问第二遍。
    let asked = site
        .seen()
        .iter()
        .filter(|seen| seen.target.starts_with("/bing"))
        .count();
    let again = search_with(
        &tool,
        json!({"query": "rust"}),
        Some(Arc::new(Picked("bing"))),
    )
    .await;
    let now = site
        .seen()
        .iter()
        .filter(|seen| seen.target.starts_with("/bing"))
        .count();
    assert_eq!(now, asked + 1, "挑的那一家只问一遍");
    assert!(
        text(&again).starts_with("Web search failed: Bing"),
        "{}",
        text(&again)
    );
}

#[tokio::test]
async fn all_sources_cooling_is_told_without_asking_anyone() {
    let site = Site::start(
        [
            "exa",
            "parallel",
            "firecrawl",
            "anysearch",
            "you",
            "duckduckgo",
            "bing",
        ]
        .map(|id| (format!("/{id}*"), Reply::status(429)))
        .to_vec(),
    )
    .await;
    let tool = tool(&site);

    let first = search(&tool, json!({"query": "rust"})).await;
    assert_eq!(
        said(&first).0,
        "web_search/failed",
        "这一次都试过了，最后一家说限额"
    );
    let asked = site.seen().len();

    let cooling = search(&tool, json!({"query": "rust"})).await;
    assert!(cooling.error);
    assert_eq!(said(&cooling).0, "web_search/cooling");
    assert_eq!(
        text(&cooling),
        "All web search sources are rate limited right now. Try again later."
    );
    assert_eq!(site.seen().len(), asked, "都冷却着，一家都没问");
    // 挑了要地址的 SearXNG 又没配地址（施工 N-3 上）：它不试，照样是「都在冷却」，不会去问一个空地址再报出错。
    let picked = search_with(
        &tool,
        json!({"query": "rust"}),
        Some(Arc::new(Picked("searxng"))),
    )
    .await;
    assert_eq!(said(&picked).0, "web_search/cooling");
}

#[tokio::test]
async fn a_success_clears_the_strikes_of_a_source() {
    // 台阶 0 秒、1 小时：第一次限额马上回来；成功一次清零，再被限还是 0 秒，不升到 1 小时
    let site = Site::start(vec![
        // 第一次：握手就 429
        ("/exa".to_string(), Reply::status(429).once()),
        // 第二次：握手、握手完的通知、调用，三个请求
        ("/exa".to_string(), mcp(EXA).once()),
        ("/exa".to_string(), mcp(EXA).once()),
        ("/exa".to_string(), mcp(EXA).once()),
        // 第三次：会话有了，只调用：429
        ("/exa".to_string(), Reply::status(429).once()),
        ("/exa".to_string(), mcp(EXA)),
        ("/parallel".to_string(), mcp("{\"results\": []}")),
    ])
    .await;
    let urls = ["exa", "parallel"]
        .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
        .to_vec();
    let searcher = Search::new(&resources())
        .testing(urls)
        .only(&["exa", "parallel"])
        .cooldown(vec![Duration::ZERO, Duration::from_secs(3600)]);
    let tool = miyu_net::tools_with(
        &resources(),
        Arc::new(Pages::new(&resources())),
        Arc::new(searcher),
    )
    .expect("字读得出")
    .into_iter()
    .find(|tool| tool.spec().name == WEB_SEARCH)
    .expect("有 web_search");

    let mut sources = Vec::new();
    for _ in 0..4 {
        let done = search(&tool, json!({"query": "tokio select"})).await;
        sources.push(said(&done).1);
    }
    assert_eq!(
        sources,
        ["Parallel", "Exa", "Parallel", "Exa"].map(|name| Some(name.to_string())),
        "握手 429 换 Parallel；回来了、成了，清零；又 429 照清零以后算还是 0 秒；没冷却照样先问 Exa。没清零的话第三次是第二级、冷却 1 小时，第四次就跳过 Exa"
    );
}

#[tokio::test]
async fn a_picked_source_is_asked_first_and_the_shipped_order_backs_it_up() {
    let site = Site::start(vec![
        ("/exa".to_string(), mcp(EXA)),
        (
            "/anysearch".to_string(),
            anysearch(json!([{"title": "One", "url": "https://a.example.org/1"}])),
        ),
        ("/you".to_string(), Reply::status(500)),
    ])
    .await;
    let tool = tool(&site);

    let picked = search_with(
        &tool,
        json!({"query": "rust"}),
        Some(Arc::new(Picked("anysearch"))),
    )
    .await;
    assert_eq!(
        said(&picked).1.as_deref(),
        Some("AnySearch"),
        "挑了的先用它"
    );
    assert_eq!(site.hits("/exa"), 0, "挑了的成了，出厂排第一的也不问");

    let broken = search_with(
        &tool,
        json!({"query": "rust"}),
        Some(Arc::new(Picked("you"))),
    )
    .await;
    assert_eq!(
        said(&broken).1.as_deref(),
        Some("Exa"),
        "挑的那一家用不了，照出厂的顺序兜底"
    );
    assert!(site.hits("/you") >= 1, "挑的那一家先问过");

    let auto = search_with(
        &tool,
        json!({"query": "rust"}),
        Some(Arc::new(Picked("auto"))),
    )
    .await;
    assert_eq!(said(&auto).1.as_deref(), Some("Exa"), "自动照出厂的顺序");
}

/// 冷却着的几家交得出来（施工 N-2 下）：设置页的下拉里写「冷却到几点」照它。还没搜过的一家都没有。
#[tokio::test]
async fn the_cooling_sources_and_their_times_are_told() {
    let site = Site::start(vec![
        ("/exa".to_string(), Reply::status(429)),
        ("/parallel".to_string(), mcp("{\"results\": []}")),
    ])
    .await;
    let urls = ["exa", "parallel"]
        .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
        .to_vec();
    let search = Arc::new(
        Search::new(&resources())
            .testing(urls)
            .only(&["exa", "parallel"]),
    );
    assert!(search.cooling().is_empty(), "还没搜过");
    let tool = miyu_net::tools_with(
        &resources(),
        Arc::new(Pages::new(&resources())),
        Arc::clone(&search),
    )
    .expect("字读得出")
    .into_iter()
    .find(|tool| tool.spec().name == WEB_SEARCH)
    .expect("有 web_search");

    let before = std::time::SystemTime::now();
    search_with(&tool, json!({"query": "rust"}), None).await;

    let cooling = search.cooling();
    assert_eq!(cooling.len(), 1, "{cooling:?}");
    assert_eq!(cooling[0].0, "exa");
    let wait = cooling[0].1.duration_since(before).expect("在以后");
    assert!(
        wait >= Duration::from_secs(29 * 60) && wait <= Duration::from_secs(31 * 60),
        "第一级台阶 30 分钟：{wait:?}"
    );
}

#[tokio::test]
async fn nothing_found_is_not_an_error() {
    let site = Site::start(vec![("/exa".to_string(), mcp("No results found."))]).await;
    let tool = tool(&site);

    let done = search(&tool, json!({"query": "zzzz"})).await;

    assert!(!done.error);
    assert_eq!(text(&done), "No results for zzzz.");
    assert_eq!(
        done.data,
        Some(json!({"kind": "links", "links": []})),
        "给脚本的搜不到也是同一个样子（施工 M-2）"
    );
    assert_eq!(
        said(&done),
        ("web_search/nothing".to_string(), Some("Exa".to_string()))
    );
}
