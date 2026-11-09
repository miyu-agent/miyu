//! 核心按需起的后台页服务（施工 BP-3，`package-pages.md`「不靠网页打开软件后台」第 3 条）：真核心、真的本机端口。`package.page`
//! 交回宿主页的地址；宿主页、后台页的文件和响应头；票只开它的包，不认识的票、别的域名、不对的方法都不答；宿主页替框调核心只认
//! 那四样、包照票认；闲够了就关，开着推送流的不算闲，关了再要重起。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use miyu_endpoint::Core;

use crate::support::extensions::{Program, install};
use crate::support::*;

/// 一个回应：状态码、头（小写的名字）、正文。
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("{}", self.text()))
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// 发一个请求（读到对面关）：`host` 是 `Host` 头。
async fn http(port: u16, method: &str, path: &str, host: &str, body: &str) -> Reply {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.expect("写得进");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("读得完");
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("有头");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("有状态码");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    }
}

/// `http://127.0.0.1:<端口>/h/<票>/` 拆成端口、票。
fn parsed(url: &str) -> (u16, String) {
    let rest = url.strip_prefix("http://127.0.0.1:").expect("本机的地址");
    let (port, path) = rest.split_once('/').expect("有路径");
    let ticket = path
        .strip_prefix("h/")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("宿主页的路径");
    (port.parse().expect("端口"), ticket.to_string())
}

/// 装一个带后台页的扩展 `id`。
fn with_page(home: &Home, id: &str, program: &Program) {
    install(home, id, &program.name(), "manual", &[]);
    let path = home.root.path().join(format!("opt/{id}/package.toml"));
    let text = std::fs::read_to_string(&path).expect("装好了");
    std::fs::write(&path, format!("{text}\n[page]\ndir = \"page\"\n")).expect("写得进");
    home.write(&format!("opt/{id}/page/index.html"), "<p>relay</p>");
    home.write(&format!("opt/{id}/page/assets/app.js"), "go()");
    home.write(&format!("opt/{id}/secret.txt"), "not for the page");
}

/// 照磁盘上的系统配置起来、装了端口的核心（包的设置项照它认），后台页服务闲了 `idle` 就关。
fn page_core(home: &Home, idle: Duration) -> Arc<Core> {
    crate::support::ported::served_with(home, |core| core.with_page_idle(idle))
}

async fn open(client: &mut Client, package: &str) -> Value {
    client
        .call("p", "package.page", json!({"package": package}))
        .await
}

async fn get(port: u16, path: String) -> Reply {
    http(port, "GET", &path, &format!("127.0.0.1:{port}"), "").await
}

async fn rpc(port: u16, ticket: &str, message: Value) -> Value {
    let reply = http(
        port,
        "POST",
        &format!("/r/{ticket}"),
        &format!("127.0.0.1:{port}"),
        &message.to_string(),
    )
    .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

#[tokio::test]
async fn the_host_serves_the_page_and_answers_for_it() {
    let home = Home::new();
    let program = Program::new();
    with_page(&home, "relay", &program);
    install(&home, "bare", &program.name(), "manual", &[]);
    let mut client = Client::connect(page_core(&home, Duration::from_secs(600)));
    client.hello().await;

    let opened = open(&mut client, "relay").await;
    let url = opened["result"]["url"]
        .as_str()
        .unwrap_or_else(|| panic!("{opened}"));
    let (port, ticket) = parsed(url);
    let here = format!("127.0.0.1:{port}");

    let host = get(port, format!("/h/{ticket}/")).await;
    assert_eq!(host.status, 200);
    assert!(
        host.header("content-type")
            .is_some_and(|kind| kind.starts_with("text/html"))
    );
    assert!(
        host.header("content-security-policy")
            .is_some_and(|csp| csp.contains("frame-ancestors 'none'"))
    );
    assert!(host.text().contains("host.js"), "{}", host.text());
    assert!(host.header("set-cookie").is_none(), "不设 cookie");
    let script = get(port, format!("/h/{ticket}/host.js")).await;
    assert!(
        script
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("text/javascript"))
    );

    let index = get(port, format!("/p/{ticket}/relay/")).await;
    assert_eq!((index.status, index.text().as_str()), (200, "<p>relay</p>"));
    assert!(
        index
            .header("content-security-policy")
            .is_some_and(|csp| csp.starts_with("sandbox allow-scripts allow-forms")
                && csp.contains("connect-src 'none'"))
    );
    assert_eq!(index.header("access-control-allow-origin"), Some("*"));
    assert_eq!(index.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(index.header("referrer-policy"), Some("no-referrer"));
    let app = get(port, format!("/p/{ticket}/relay/assets/app.js")).await;
    assert_eq!((app.status, app.text().as_str()), (200, "go()"));

    for (path, code) in [
        (format!("/p/{ticket}/bare/"), 404),
        (format!("/p/{ticket}/relay/../secret.txt"), 404),
        (format!("/p/{ticket}/relay/none.js"), 404),
        (format!("/h/{}/", "0".repeat(64)), 404),
    ] {
        assert_eq!(get(port, path.clone()).await.status, code, "{path}");
    }
    let elsewhere = http(
        port,
        "GET",
        &format!("/h/{ticket}/"),
        &format!("evil.example:{port}"),
        "",
    )
    .await;
    assert_eq!(elsewhere.status, 421, "借别的域名指过来的不答");
    let posted = http(port, "POST", &format!("/h/{ticket}/"), &here, "").await;
    assert_eq!(posted.status, 405);

    let context = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 1, "method": "context"}),
    )
    .await;
    assert_eq!(
        context["result"],
        json!({"package": "relay", "language": "zh"}),
        "{context}"
    );
    let other = rpc(port, &ticket, json!({"jsonrpc": "2.0", "id": 2, "method": "settings.set", "params": {"changes": [{"key": "bare.x", "value": 1}]}})).await;
    assert_eq!(
        other["error"]["data"]["reason"], "forbidden",
        "键要是它自己的：{other}"
    );
    let call = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 3, "method": "call", "params": {"method": "echo"}}),
    )
    .await;
    assert_eq!(
        call["error"]["data"]["reason"], "program_not_running",
        "照票认的包转过去：{call}"
    );
    let unknown = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 4, "method": "config.get", "params": {}}),
    )
    .await;
    assert_eq!(unknown["error"]["code"], -32601, "只认那四样：{unknown}");
    let settings = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 5, "method": "settings.get"}),
    )
    .await;
    assert_eq!(
        settings["result"],
        json!({"items": [], "values": {}}),
        "没有设置项：{settings}"
    );

    for (package, reason) in [("bare", "no_page"), ("nothing", "unknown_package")] {
        let refused = open(&mut client, package).await;
        assert_eq!(
            refused["error"]["data"]["reason"], reason,
            "{package}：{refused}"
        );
    }
    let again = open(&mut client, "relay").await;
    let (same, another) = parsed(again["result"]["url"].as_str().unwrap_or_default());
    assert_eq!(same, port, "开着的服务照用");
    assert_ne!(another, ticket, "每要一次一张新票");
}

/// 闲够了就关、票作废；开着推送流的不算闲；关了再要重起。
#[tokio::test]
async fn an_idle_host_closes_and_starts_again_when_asked() {
    let home = Home::new();
    let program = Program::new();
    with_page(&home, "relay", &program);
    let mut client = Client::connect(page_core(&home, Duration::from_millis(300)));
    client.hello().await;
    let opened = open(&mut client, "relay").await;
    let (port, ticket) = parsed(
        opened["result"]["url"]
            .as_str()
            .unwrap_or_else(|| panic!("{opened}")),
    );

    // 推送流开着：过了好几个闲置期照样答。
    let mut events = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let request = format!("GET /e/{ticket} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    events.write_all(request.as_bytes()).await.expect("写得进");
    let mut head = [0u8; 512];
    let read = events.read(&mut head).await.expect("有回应");
    let head = String::from_utf8_lossy(&head[..read]).to_ascii_lowercase();
    assert!(
        head.starts_with("http/1.1 200") && head.contains("text/event-stream"),
        "{head}"
    );
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let still = http(
        port,
        "GET",
        &format!("/h/{ticket}/"),
        &format!("127.0.0.1:{port}"),
        "",
    )
    .await;
    assert_eq!(still.status, 200, "开着推送流的不算闲");

    drop(events);
    let closed = tokio::time::timeout(Duration::from_secs(30), async {
        while tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(closed.is_ok(), "闲够了就关");

    let reopened = open(&mut client, "relay").await;
    let (port, ticket) = parsed(
        reopened["result"]["url"]
            .as_str()
            .unwrap_or_else(|| panic!("{reopened}")),
    );
    let page = http(
        port,
        "GET",
        &format!("/p/{ticket}/relay/"),
        &format!("127.0.0.1:{port}"),
        "",
    )
    .await;
    assert_eq!(
        (page.status, page.text().as_str()),
        (200, "<p>relay</p>"),
        "再要重起"
    );
}

/// 宿主页替框读写它自己的设置项（施工 BP-3）：只给它自己的、没写的照默认；写进系统配置，再读是新的值。包经 `package.install` 装上，
/// 设置项才进配置清单。
#[tokio::test]
async fn the_host_reads_and_writes_only_the_page_s_own_settings() {
    let home = Home::new();
    let program = Program::new();
    let folder = home.work.join("tuned");
    std::fs::create_dir_all(folder.join("page")).expect("建得了");
    std::fs::write(folder.join("page/index.html"), "<p>tuned</p>").expect("写得进");
    std::fs::write(
        folder.join("package.toml"),
        format!("[package]\nprotocol = [1, 1]\nname = {{ en = \"Tuned\" }}\n\n[command]\nname = \"tuned\"\nprogram = \"{}\"\nabout = {{ en = \"T\" }}\n\n[process]\nstart = \"manual\"\n\n[page]\ndir = \"page\"\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\"]\nname = {{ en = \"Port\" }}\n", program.name()),
    )
    .expect("写得进");
    let mut client = Client::connect(page_core(&home, Duration::from_secs(600)));
    client.hello().await;
    let installed = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "tuned", "{installed}");
    let opened = open(&mut client, "tuned").await;
    let (port, ticket) = parsed(
        opened["result"]["url"]
            .as_str()
            .unwrap_or_else(|| panic!("{opened}")),
    );

    let settings = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 1, "method": "settings.get"}),
    )
    .await;
    let keys: Vec<&Value> = settings["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{settings}"))
        .iter()
        .map(|item| &item["key"])
        .collect();
    // 同网页 `frame.js`：照配置清单的 `package` 认，核心替每个包加的「只对我关掉」也算它的。
    assert_eq!(
        keys,
        [&json!("packages.tuned.off"), &json!("tuned.port")],
        "只有它自己的项：{settings}"
    );
    assert_eq!(
        settings["result"]["values"]["tuned.port"], 8400,
        "没写的照默认"
    );
    let set = rpc(port, &ticket, json!({"jsonrpc": "2.0", "id": 2, "method": "settings.set", "params": {"changes": [{"key": "tuned.port", "value": 9}]}})).await;
    assert!(set.get("error").is_none(), "{set}");
    let again = rpc(
        port,
        &ticket,
        json!({"jsonrpc": "2.0", "id": 3, "method": "settings.get"}),
    )
    .await;
    assert_eq!(
        again["result"]["values"]["tuned.port"], 9,
        "写进系统配置：{again}"
    );
}
