//! `web_fetch` 在本机的假服务器上整条走通（施工 N-1，设计 36 第一节第 3、4 条，`net.md`「抓网页」）：网页换成 Markdown、
//! 文字照原样、GBK 照字符集解、太长的分页、翻页不重抓、跳转写落到了哪；本机内网的地址、不是 http 的、4xx、图这些说一句
//! 抓不了。假服务器在回环上，用测试的口子（`testkit`）把回环当公网。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::Access;
use miyu_net::testkit::Testing;
use miyu_net::{Pages, WEB_FETCH};
use miyu_tool::{Call, Done, Progress, Stop, Tool};

use crate::support::{PNG, Reply, Site, local, resources};

/// 打开 `testing` 这个口子的 `web_fetch`。
fn tool(testing: Testing) -> Arc<dyn Tool> {
    let pages = Pages::new(&resources()).testing(testing);
    let search = miyu_net::Search::new(&resources());
    let tools =
        miyu_net::tools_with(&resources(), Arc::new(pages), Arc::new(search)).expect("字读得出");
    tools
        .into_iter()
        .find(|tool| tool.spec().name == WEB_FETCH)
        .expect("有 web_fetch")
}

/// 调一次，等最多一分钟。
pub(crate) async fn fetch(tool: &Arc<dyn Tool>, args: serde_json::Value) -> Done {
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
        packages: None,
        ids: None,
        settings: None,
    };
    tokio::time::timeout(
        Duration::from_secs(60),
        tool.run(call, Progress::new(|_| {})),
    )
    .await
    .expect("一分钟内抓完")
}

/// 交回的字。
pub(crate) fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 给人看的说法的编号，去掉 `software/net/`。
fn said(done: &Done) -> String {
    let key = done
        .human
        .as_ref()
        .map(|said| said.key.clone())
        .unwrap_or_default();
    key.trim_start_matches("software/net/").to_string()
}

#[tokio::test]
async fn the_tool_is_read_only_and_needs_only_a_url() {
    let tool = tool(local());
    let spec = tool.spec();
    assert_eq!(spec.name, WEB_FETCH);
    assert_eq!(
        spec.access,
        Access::Read,
        "只读：哪里都不问人（设计 36 第一节第 4 条）"
    );
    let parameters: serde_json::Value = serde_json::from_str(spec.parameters.get()).unwrap();
    assert_eq!(parameters["required"], serde_json::json!(["url"]));
}

#[tokio::test]
async fn a_web_page_is_read_as_markdown_with_its_title_for_people() {
    let html = r#"<html><head><title>Miyu &amp; friends</title><script>var secret = 1;</script></head>
        <body><nav>menu</nav><h1>Hello</h1><p>A <a href="/next">link</a>.</p></body></html>"#;
    let site = Site::start(vec![("/page".to_string(), Reply::html(html))]).await;
    let tool = tool(local());

    let done = fetch(
        &tool,
        serde_json::json!({"url": format!("  {}  ", site.url("/page"))}),
    )
    .await;

    assert!(!done.error, "{}", text(&done));
    let read = text(&done);
    assert!(read.starts_with("# Hello"), "{read}");
    assert!(read.contains("[link](/next)"), "{read}");
    assert!(!read.contains("secret") && !read.contains("menu"), "{read}");
    assert!(!read.contains("Page 1"), "一页的不写第几页：{read}");
    assert_eq!(said(&done), "web_fetch/titled");
    // 照规矩里 `document` 的那一份（也收纯文字、JSON），不是卡片的那一份
    assert_eq!(
        site.seen()[0].accept,
        "text/html,application/xhtml+xml,text/plain;q=0.9,application/json;q=0.8,*/*;q=0.5"
    );
    let human = done.human.as_ref().unwrap();
    assert_eq!(
        human.fields.get("title").map(String::as_str),
        Some("Miyu & friends")
    );
}

#[tokio::test]
async fn text_and_json_are_read_as_they_are_and_gbk_is_decoded() {
    let json = r#"{"a": [1, 2]}"#;
    let gbk = b"<html><head><meta charset=gbk></head><body><p>\xd6\xd0\xce\xc4</p></body></html>";
    let site = Site::start(vec![
        (
            "/data.json".to_string(),
            Reply::bytes("application/json", json.as_bytes()),
        ),
        ("/old".to_string(), Reply::bytes("text/html", gbk)),
    ])
    .await;
    let tool = tool(local());

    let done = fetch(&tool, serde_json::json!({"url": site.url("/data.json")})).await;
    assert_eq!(text(&done), json);
    assert_eq!(said(&done), "web_fetch/fetched", "没有标题的说抓到了");

    let done = fetch(&tool, serde_json::json!({"url": site.url("/old")})).await;
    assert_eq!(text(&done), "中文");
}

#[tokio::test]
async fn a_long_page_is_paged_and_turning_pages_does_not_fetch_again() {
    // 一行 100 个字，一千行：十万字，每页三万字，四页
    let line = format!("{}\n", "x".repeat(99));
    let body = line.repeat(1000);
    let site = Site::start(vec![(
        "/long.txt".to_string(),
        Reply::bytes("text/plain", body.as_bytes()),
    )])
    .await;
    let tool = tool(local());
    let url = site.url("/long.txt");

    let first = fetch(&tool, serde_json::json!({"url": url})).await;
    let first_text = text(&first);
    assert!(
        first_text.ends_with("(Page 1 of 4.)"),
        "{}",
        &first_text[first_text.len() - 40..]
    );
    assert_eq!(said(&first), "web_fetch/page");

    let mut whole = String::new();
    for page in 1..=4 {
        let done = fetch(&tool, serde_json::json!({"url": url, "page": page})).await;
        let read = text(&done);
        let footer = format!("\n(Page {page} of 4.)");
        let body = read
            .strip_suffix(&footer)
            .unwrap_or_else(|| panic!("第 {page} 页的末尾：{read}"));
        assert!(body.chars().count() <= 30_000);
        whole.push_str(body);
        whole.push('\n');
    }
    assert_eq!(whole, body, "四页拼回去一个字不差");
    assert_eq!(site.hits("/long.txt"), 1, "十分钟内翻页照记着的");

    // 两页的也写第几页
    let two = format!("{}\n", "z".repeat(99)).repeat(400);
    let site_two = Site::start(vec![(
        "/two.txt".to_string(),
        Reply::bytes("text/plain", two.as_bytes()),
    )])
    .await;
    let read = text(&fetch(&tool, serde_json::json!({"url": site_two.url("/two.txt")})).await);
    assert!(
        read.ends_with("\n(Page 1 of 2.)"),
        "{}",
        &read[read.len() - 40..]
    );

    let done = fetch(&tool, serde_json::json!({"url": url, "page": 5})).await;
    assert!(done.error);
    assert_eq!(text(&done), "There is no page 5. It has 4.");
    let done = fetch(&tool, serde_json::json!({"url": url, "page": 0})).await;
    assert_eq!(said(&done), "web_fetch/no-page");
}

#[tokio::test]
async fn the_charset_in_the_header_is_used() {
    let site = Site::start(vec![(
        "/gbk.txt".to_string(),
        Reply::bytes("text/plain; charset=\"GBK\"", b"\xd6\xd0\xce\xc4"),
    )])
    .await;
    let tool = tool(local());

    let done = fetch(&tool, serde_json::json!({"url": site.url("/gbk.txt")})).await;

    assert_eq!(text(&done), "中文");
}

#[tokio::test]
async fn a_page_over_the_limit_is_cut_and_the_last_page_says_so() {
    // 上限 5 MiB：六百万个字节，一行 100 个
    let body = format!("{}\n", "y".repeat(99)).repeat(60_000);
    let site = Site::start(vec![(
        "/huge.txt".to_string(),
        Reply::bytes("text/plain", body.as_bytes()),
    )])
    .await;
    let tool = tool(local());
    let url = site.url("/huge.txt");

    let first = text(&fetch(&tool, serde_json::json!({"url": url})).await);
    let pages: usize = first
        .rsplit_once(" of ")
        .and_then(|(_, tail)| tail.trim_end_matches(".)").parse().ok())
        .unwrap_or_else(|| panic!("末尾写着一共几页：{}", &first[first.len() - 40..]));
    assert_eq!(
        pages,
        (5 * 1024 * 1024usize).div_ceil(30_000),
        "截在 5 MiB 上，每页三万字"
    );
    assert!(!first.contains("too large"), "不是最后一页的不说");

    let last = text(&fetch(&tool, serde_json::json!({"url": url, "page": pages})).await);
    assert!(
        last.ends_with("\n(The rest of the page was too large to fetch.)"),
        "{}",
        &last[last.len() - 80..]
    );
}

#[tokio::test]
async fn a_bot_check_page_is_not_read_as_content() {
    let site = Site::start(vec![(
        "/guarded".to_string(),
        Reply::html(
            "<html><head><title>Just a moment...</title></head><body>checking</body></html>",
        )
        .with_status(503)
        .with_header("cf-mitigated", "challenge"),
    )])
    .await;
    let tool = tool(local());

    let done = fetch(&tool, serde_json::json!({"url": site.url("/guarded")})).await;

    assert!(done.error);
    assert_eq!(said(&done), "web_fetch/challenge");
}

#[tokio::test]
async fn a_redirect_says_where_it_landed() {
    let site = Site::start(vec![
        ("/old".to_string(), Reply::redirect("/new")),
        ("/new".to_string(), Reply::bytes("text/plain", b"here")),
    ])
    .await;
    let tool = tool(local());

    let done = fetch(&tool, serde_json::json!({"url": site.url("/old")})).await;

    assert_eq!(
        text(&done),
        format!("Redirected to {}.\n\nhere", site.url("/new"))
    );
}

#[tokio::test]
async fn what_cannot_be_read_says_why() {
    let site = Site::start(vec![
        ("/gone".to_string(), Reply::status(404)),
        ("/card.png".to_string(), Reply::bytes("image/png", PNG)),
        (
            "/blank".to_string(),
            Reply::html("<html><body><script>x()</script></body></html>"),
        ),
    ])
    .await;
    let tool = tool(local());
    let cases = [
        (
            site.url("/gone"),
            "web_fetch/status",
            format!("{} answered with HTTP 404.", site.url("/gone")),
        ),
        (
            site.url("/card.png"),
            "web_fetch/unsupported",
            format!(
                "{} is image/png, which cannot be read as text.",
                site.url("/card.png")
            ),
        ),
        (
            site.url("/blank"),
            "web_fetch/empty",
            format!("{} has no readable text.", site.url("/blank")),
        ),
        (
            "ftp://site.test/file".to_string(),
            "web_fetch/not-a-url",
            "ftp://site.test/file is not an http or https URL.".to_string(),
        ),
        (
            "http://inner.test/".to_string(),
            "web_fetch/blocked",
            "http://inner.test/ is a local or private network address and cannot be fetched."
                .to_string(),
        ),
    ];
    for (url, key, expected) in cases {
        let done = fetch(&tool, serde_json::json!({"url": url})).await;
        assert!(done.error, "{url}");
        assert_eq!(said(&done), key, "{url}");
        assert_eq!(text(&done), expected);
    }
    let done = fetch(&tool, serde_json::json!({"page": 1})).await;
    assert_eq!(said(&done), "web_fetch/bad-args");
}

#[tokio::test]
async fn without_the_testing_gate_the_loopback_server_is_blocked() {
    let site = Site::start(vec![("/page".to_string(), Reply::html("<p>hi</p>"))]).await;
    let tool = tool(Testing::default());

    let done = fetch(&tool, serde_json::json!({"url": site.url("/page")})).await;

    assert_eq!(said(&done), "web_fetch/blocked");
    assert_eq!(site.hits("/page"), 0, "闸在连之前");
}
