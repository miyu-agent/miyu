//! 扩展的搜索来源（施工 N-5，设计 36 第二节第 5 条）：选了 `<包>/<编号>` 的先问扩展（经装进来的端口），它答了的照它、给人写
//! 它的名字；它说限额的冷却、下一次不问、照出厂的顺序兜底，冷却着的在 `cooling` 里；没选它的、选了出厂那几家的从不问它；
//! 端口没装的照出厂的顺序。出厂的几家指到假服务器上。

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use miyu_net::{Answered, Found, Hit, Missed, Query, Search, SearchPort, Unanswered, Using};

use crate::support::{Reply, Site, resources};

/// 假的端口：照 `answer` 答，数着被问了几次、问的是哪个来源、替谁问的。
struct Port {
    asked: AtomicUsize,
    sources: Mutex<Vec<String>>,
    owners: Mutex<Vec<Option<String>>>,
    answer: Result<Answered, Unanswered>,
}

impl SearchPort for Port {
    fn query<'a>(
        &'a self,
        (source, owner): (&'a str, Option<&'a miyu_kernel::id::AccountId>),
        _query: &'a Query,
        _want: usize,
    ) -> Pin<Box<dyn Future<Output = Result<Answered, Unanswered>> + Send + 'a>> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.sources
            .lock()
            .expect("锁没坏")
            .push(source.to_string());
        self.owners
            .lock()
            .expect("锁没坏")
            .push(owner.map(|owner| owner.as_str().to_string()));
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

fn port(answer: Result<Answered, Unanswered>) -> Arc<Port> {
    Arc::new(Port {
        asked: AtomicUsize::new(0),
        sources: Mutex::new(Vec::new()),
        owners: Mutex::new(Vec::new()),
        answer,
    })
}

fn found_by(name: &str) -> Result<Answered, Unanswered> {
    Ok(Answered {
        name: name.to_string(),
        hits: vec![Hit {
            title: "From the extension".to_string(),
            url: "https://ext.example/".to_string(),
            snippet: None,
            date: None,
        }],
    })
}

/// 出厂的几家都指到假服务器：Exa 答一条，别的 500。
async fn shipped() -> (Site, Search) {
    let text = "Title: Exa hit\nURL: https://exa.example/\nPublished: N/A\nAuthor: N/A\nHighlights:\nfrom exa\n";
    let body =
        json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": text}]}});
    let site = Site::start(vec![(
        "/exa".to_string(),
        Reply::bytes("application/json", body.to_string().as_bytes()),
    )])
    .await;
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
    (site, search)
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

#[tokio::test]
async fn a_picked_extension_source_is_asked_first() {
    let (_site, search) = shipped().await;
    let port = port(found_by("Finder"));
    search.attach(port.clone());
    let found = ask(&search, &Using::picked("finder/web"))
        .await
        .expect("搜得到");
    assert_eq!(found.source, "Finder", "给人写它的名字");
    assert_eq!(found.hits[0].url, "https://ext.example/");
    assert_eq!(*port.sources.lock().expect("锁没坏"), ["finder/web"]);

    let found = ask(&search, &Using::picked("auto")).await.expect("搜得到");
    assert_eq!(found.source, "Exa");
    let found = ask(&search, &Using::picked("exa")).await.expect("搜得到");
    assert_eq!(found.source, "Exa");
    assert_eq!(port.asked.load(Ordering::SeqCst), 1, "没选它的不问它");
}

#[tokio::test]
async fn a_limited_extension_cools_down_and_the_shipped_order_takes_over() {
    let (_site, search) = shipped().await;
    let port = port(Err(Unanswered::Limited));
    search.attach(port.clone());
    let using = Using::picked("finder/web");
    assert_eq!(
        ask(&search, &using).await.expect("搜得到").source,
        "Exa",
        "兜底"
    );
    assert!(
        search.cooling().iter().any(|(id, _)| id == "finder/web"),
        "冷却着的在下拉的状态里"
    );
    assert_eq!(ask(&search, &using).await.expect("搜得到").source, "Exa");
    assert_eq!(port.asked.load(Ordering::SeqCst), 1, "冷却着的不问");

    let failing = port_failing();
    let (_other, search) = shipped().await;
    search.attach(failing.clone());
    assert_eq!(ask(&search, &using).await.expect("搜得到").source, "Exa");
    assert_eq!(ask(&search, &using).await.expect("搜得到").source, "Exa");
    assert_eq!(failing.asked.load(Ordering::SeqCst), 2, "出错的不冷却");
}

fn port_failing() -> Arc<Port> {
    port(Err(Unanswered::Failed("not running".to_string())))
}

#[tokio::test]
async fn without_a_port_the_shipped_order_answers() {
    let (_site, search) = shipped().await;
    let found = ask(&search, &Using::picked("finder/web"))
        .await
        .expect("搜得到");
    assert_eq!(found.source, "Exa");
}

/// 设置里挑了 `source` 的那一轮的配置。
struct Picked(&'static str);

impl miyu_tool::SettingsPort for Picked {
    fn values(&self) -> miyu_config::Values {
        let mut values = miyu_config::Values::defaults(miyu_net::SearchSettings::ITEMS);
        values.set("net.search.source", miyu_config::Value::Text(self.0.into()));
        values
    }

    fn secret(
        &self,
        _reference: &miyu_config::secret::Reference,
    ) -> Option<miyu_config::secret::Secret> {
        None
    }
}

/// 她在 bob 的会话里用 `web_search`：问扩展的来源时带上 bob（施工 IM-1 中下三：每人一份的包问他那一份）。
#[tokio::test]
async fn the_tool_asks_the_extension_for_the_session_s_owner() {
    let (_site, search) = shipped().await;
    let port = port(found_by("Finder"));
    search.attach(port.clone());
    let tool = miyu_net::tools_with(
        &resources(),
        Arc::new(miyu_net::Pages::new(&resources())),
        Arc::new(search),
    )
    .expect("字读得出")
    .into_iter()
    .find(|tool| tool.spec().name == miyu_net::WEB_SEARCH)
    .expect("有 web_search");
    let ids = miyu_tool::CallIds {
        session: miyu_kernel::id::SessionId::parse("01890000-0000-7000-8000-000000000001")
            .expect("合写法"),
        owner: miyu_kernel::id::AccountId::parse("bob").expect("合写法"),
        call: miyu_kernel::id::CallId::parse("call_1_1").expect("合写法"),
        asked: None,
        preset: None,
    };
    let call = miyu_tool::Call {
        args: json!({"query": "rust"}).to_string(),
        settings: Some(Arc::new(Picked("finder/web"))),
        ids: Some(ids),
        ..miyu_tool::testkit::call("{}")
    };
    let done = tokio::time::timeout(
        Duration::from_secs(60),
        tool.run(call, miyu_tool::Progress::new(|_| {})),
    )
    .await
    .expect("一分钟内搜完");
    assert!(!done.error, "搜得到");
    assert_eq!(
        *port.owners.lock().expect("锁没坏"),
        [Some("bob".to_string())]
    );
}
