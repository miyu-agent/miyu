//! 联网服务选供应商（施工 NF-1，项目主人 2026-11-14 定）：选了 DeepSeek 的，搜索走 DeepSeek Search，自己抓不下来的网页交
//! DeepSeek Fetch；它限额（429）、余额不足（402）、服务忙（503）、没配 key 的照自动的顺序退回免费的几家；自动的照旧不问
//! DeepSeek。key 照模型那边配了 DeepSeek 的那一家供应商取。几家的地址照 `Search::testing` 换成本机的假服务器，不出网。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_config::Value as ConfigValue;
use miyu_config::Values;
use miyu_config::secret::{Reference, Secret};
use miyu_net::{Pages, Search, WEB_FETCH, WEB_SEARCH};
use miyu_tool::{Call, Done, SettingsPort, Tool};

use crate::support::{Reply, Site, local, resources};

/// 出厂顺序里的几家，都指到假服务器的 `/<编号>`。
const SOURCES: [&str; 8] = [
    "exa",
    "parallel",
    "firecrawl",
    "anysearch",
    "you",
    "deepseek",
    "duckduckgo",
    "bing",
];

/// 模型那边 DeepSeek 供应商的 key。
const PROVIDER_KEY: &str = "sk-provider";

/// 这一轮的配置：联网服务选了 `0`；`1` 是真的，模型那边配了一家 DeepSeek 供应商（地址是官方的、key 照引用取）。
struct Turn(&'static str, bool);

impl SettingsPort for Turn {
    fn values(&self) -> Values {
        let mut values = Values::defaults(&miyu_net::ITEMS.concat());
        values.set("net.search.source", ConfigValue::Text(self.0.into()));
        if self.1 {
            values.set(
                "providers.ds.base_url",
                ConfigValue::Text("https://api.deepseek.com".into()),
            );
            values.set(
                "providers.ds.key",
                ConfigValue::Secret(Reference::Secret("ds".to_string())),
            );
        }
        values
    }

    fn secret(&self, reference: &Reference) -> Option<Secret> {
        (*reference == Reference::Secret("ds".to_string()))
            .then(|| Secret::new(PROVIDER_KEY).ok())
            .flatten()
    }
}

/// 一份 MCP 的工具结果：握手也回它，客户端只看状态码。
fn mcp(text: &str) -> Reply {
    let body =
        json!({"jsonrpc": "2.0", "id": 2, "result": {"content": [{"type": "text", "text": text}]}});
    Reply::bytes("application/json", body.to_string().as_bytes())
}

fn reply(value: Value) -> Reply {
    Reply::bytes("application/json", value.to_string().as_bytes())
}

/// 一段够长的正文。
fn body(word: &str) -> String {
    format!("{word} ").repeat(80)
}

/// Exa 搜、抓都回的一段字：搜的读出一条，抓的去掉开头两行是正文。
fn exa() -> Reply {
    mcp(&format!(
        "Title: Exa hit\nURL: https://exa.example/\n\n{}",
        body("exa")
    ))
}

/// DeepSeek 的出错（OpenAI 那种 JSON），状态码是 `status`。
fn deepseek_error(status: u16) -> Reply {
    reply(json!({"error": {"message": "Service is busy", "type": "unknown_error"}}))
        .with_status(status)
}

/// 网页本身被挡（403），Exa 和 DeepSeek 照 `exa`、`search`、`fetch` 回。
async fn site(search: Reply, fetch: Reply) -> Site {
    Site::start(vec![
        ("/page".to_string(), Reply::status(403)),
        ("/exa".to_string(), exa()),
        ("/deepseek".to_string(), search),
        ("/deepseek-fetch".to_string(), fetch),
    ])
    .await
}

fn deepseek_found() -> Reply {
    reply(json!({"results": [{"title": "D", "url": "https://d.example/", "summary": "s"}]}))
}

fn deepseek_fetched() -> Reply {
    reply(json!({"title": "", "content": body("ds")}))
}

/// 两件工具，用同一份搜索的家底（冷却共用）。
fn tools(site: &Site) -> (Arc<dyn Tool>, Arc<dyn Tool>) {
    let mut urls: Vec<(String, String)> = SOURCES
        .map(|id| (id.to_string(), site.url(&format!("/{id}"))))
        .to_vec();
    urls.push(("deepseek:fetch".to_string(), site.url("/deepseek-fetch")));
    let pages = Pages::new(&resources()).testing(local());
    let search = Search::new(&resources()).testing(urls);
    let all =
        miyu_net::tools_with(&resources(), Arc::new(pages), Arc::new(search)).expect("字读得出");
    let pick = |name: &str| {
        all.iter()
            .find(|tool| tool.spec().name == name)
            .cloned()
            .expect("有这件工具")
    };
    (pick(WEB_SEARCH), pick(WEB_FETCH))
}

async fn run(tool: &Arc<dyn Tool>, args: Value, turn: Turn) -> Done {
    let settings: Arc<dyn SettingsPort> = Arc::new(turn);
    let call = Call {
        settings: Some(settings),
        ..crate::fetch::call(args)
    };
    crate::fetch::run(tool, call).await
}

/// 给人看的那一句里是哪一家。
fn via(done: &Done) -> Option<String> {
    done.human
        .as_ref()
        .expect("有给人看的一句")
        .fields
        .get("via")
        .cloned()
}

#[tokio::test]
async fn deepseek_searches_and_fetches_with_the_model_provider_key() {
    let site = site(deepseek_found(), deepseek_fetched()).await;
    let (search, fetch) = tools(&site);

    let found = run(&search, json!({"query": "rust"}), Turn("deepseek", true)).await;
    assert_eq!(via(&found).as_deref(), Some("DeepSeek"), "搜索走 DeepSeek");
    let page = site.url("/page");
    let fetched = run(&fetch, json!({"url": page}), Turn("deepseek", true)).await;
    assert!(!fetched.error, "{fetched:?}");
    assert_eq!(
        via(&fetched).as_deref(),
        Some("DeepSeek"),
        "代抓走 DeepSeek"
    );

    assert_eq!(site.hits("/exa"), 0, "免费的几家一家都没问");
    let seen = site.seen();
    let asked = seen
        .iter()
        .find(|seen| seen.target == "/deepseek-fetch")
        .expect("问了 DeepSeek Fetch");
    assert_eq!(asked.method, "POST");
    assert_eq!(
        asked.authorization,
        format!("Bearer {PROVIDER_KEY}"),
        "key 照模型那边的 DeepSeek 供应商"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&asked.body).expect("正文是 JSON"),
        json!({"url": page})
    );
}

#[tokio::test]
async fn deepseek_failing_falls_back_to_the_free_ones() {
    for status in [429, 402, 503] {
        let site = site(deepseek_error(status), deepseek_error(status)).await;
        let (search, _) = tools(&site);
        let found = run(&search, json!({"query": "rust"}), Turn("deepseek", true)).await;
        assert_eq!(
            via(&found).as_deref(),
            Some("Exa"),
            "搜索 {status} 退回免费的"
        );
        assert_eq!(site.hits("/deepseek"), 1, "{status}");

        let site = self::site(deepseek_error(status), deepseek_error(status)).await;
        let (_, fetch) = tools(&site);
        let page = site.url("/page");
        let fetched = run(&fetch, json!({"url": page}), Turn("deepseek", true)).await;
        assert!(!fetched.error, "{status}：{fetched:?}");
        assert_eq!(
            via(&fetched).as_deref(),
            Some("Exa"),
            "代抓 {status} 退回免费的"
        );
        assert_eq!(site.hits("/deepseek-fetch"), 1, "先问过 DeepSeek：{status}");
    }
}

#[tokio::test]
async fn a_limit_rests_deepseek_for_both_searching_and_fetching() {
    let site = site(deepseek_error(429), deepseek_fetched()).await;
    let (search, fetch) = tools(&site);
    let found = run(&search, json!({"query": "rust"}), Turn("deepseek", true)).await;
    assert_eq!(via(&found).as_deref(), Some("Exa"));
    let fetched = run(
        &fetch,
        json!({"url": site.url("/page")}),
        Turn("deepseek", true),
    )
    .await;
    assert_eq!(via(&fetched).as_deref(), Some("Exa"));
    assert_eq!(site.hits("/deepseek-fetch"), 0, "搜索报了限额，抓也一起歇");
}

#[tokio::test]
async fn auto_never_asks_deepseek() {
    // 免费的几家都不成，模型那边配了 DeepSeek 供应商：自动也一次不问 DeepSeek（项目主人定：自动不花钱）。
    let mut routes: Vec<(String, Reply)> = SOURCES
        .iter()
        .filter(|id| **id != "deepseek")
        .map(|id| (format!("/{id}"), Reply::status(500)))
        .collect();
    routes.extend([
        ("/page".to_string(), Reply::status(403)),
        ("/deepseek".to_string(), deepseek_found()),
        ("/deepseek-fetch".to_string(), deepseek_fetched()),
    ]);
    let site = Site::start(routes).await;
    let (search, fetch) = tools(&site);
    let found = run(&search, json!({"query": "rust"}), Turn("auto", true)).await;
    assert!(found.error, "免费的都不成就是搜不成：{found:?}");
    let fetched = run(
        &fetch,
        json!({"url": site.url("/page")}),
        Turn("auto", true),
    )
    .await;
    assert!(fetched.error, "免费的都不成就照自己抓的原因：{fetched:?}");
    assert_eq!(
        site.hits("/deepseek") + site.hits("/deepseek-fetch"),
        0,
        "自动只用免费额度，有 key 也不问 DeepSeek"
    );
}

#[tokio::test]
async fn without_a_key_deepseek_is_skipped() {
    let site = site(deepseek_found(), deepseek_fetched()).await;
    let (search, fetch) = tools(&site);
    let found = run(&search, json!({"query": "rust"}), Turn("deepseek", false)).await;
    assert_eq!(via(&found).as_deref(), Some("Exa"));
    let fetched = run(
        &fetch,
        json!({"url": site.url("/page")}),
        Turn("deepseek", false),
    )
    .await;
    assert_eq!(via(&fetched).as_deref(), Some("Exa"));
    assert_eq!(
        site.hits("/deepseek") + site.hits("/deepseek-fetch"),
        0,
        "没 key 的不问，直接退回"
    );
}

/// 自己抓得下来的一页。
fn own_page() -> Reply {
    Reply::html(&format!(
        "<html><head><title>Own</title></head><body><p>{}</p></body></html>",
        body("own")
    ))
}

/// 选了带代抓的服务（施工 NF-1 补，项目主人 2026-11-14 定）：先交它代抓，本机直抓不碰。
#[tokio::test]
async fn a_picked_provider_fetches_before_her_own_try() {
    let site = site(deepseek_found(), deepseek_fetched()).await;
    let own = Site::start(vec![("/good".to_string(), own_page())]).await;
    let (_, fetch) = tools(&site);
    let page = own.url("/good");
    let fetched = run(&fetch, json!({"url": page}), Turn("deepseek", true)).await;
    assert!(!fetched.error, "{fetched:?}");
    assert_eq!(via(&fetched).as_deref(), Some("DeepSeek"));
    assert_eq!(
        site.hits("/deepseek-fetch"),
        1,
        "第一次请求就交 DeepSeek Fetch"
    );
    assert_eq!(own.hits("/good"), 0, "本机直抓没被调用");
}

/// DeepSeek 代抓失败了才自己抓；自己抓得下来的不再问免费的几家。
#[tokio::test]
async fn her_own_try_comes_after_the_provider_fails() {
    let site = site(deepseek_found(), deepseek_error(503)).await;
    let own = Site::start(vec![("/good".to_string(), own_page())]).await;
    let (_, fetch) = tools(&site);
    let fetched = run(
        &fetch,
        json!({"url": own.url("/good")}),
        Turn("deepseek", true),
    )
    .await;
    assert!(!fetched.error, "{fetched:?}");
    assert_eq!(via(&fetched), None, "自己抓的");
    assert_eq!(site.hits("/deepseek-fetch"), 1, "先问过 DeepSeek");
    assert_eq!(own.hits("/good"), 1);
    assert_eq!(site.hits("/exa"), 0, "自己抓成了，不找免费的几家");
}
