//! 重场景里本机的假服务（施工 LW-30）：一个口，只听 `127.0.0.1`，照请求行分。
//!
//! - **网页的代理**：请求行写着整个网址、主机以 `.invalid` 结尾的（`http://pages.perf.invalid/a.html`），交一页 HTML（标题、
//!   表格、列表、代码、引用、链接、长段的字）：`web_fetch`、`link.preview` 经代理来抓。地址闸不放回环，本机解析不出来的名字
//!   照样交给代理（`internal/blueprint/net.md` 第 5 条），所以网址写解析不出来的名字、核心的 `HTTP_PROXY` 指到这里。
//! - **SearXNG**：`GET /search?q=&format=json`。
//! - **MCP 网址服务**：`POST /mcp` 直接答；`POST /mcp2` 要登录，没带对的令牌回 `401`，走 OAuth 的发现
//!   （`/.well-known/oauth-protected-resource`、`/.well-known/oauth-authorization-server`）、动态注册（`/register`）、授权页
//!   （`/authorize` 带着 `code` 跳回核心的回调）、换令牌（`/token`）。答法照 `heavy/mcp.rs`。
//!
//! 一个连接只答一个请求，答完关掉（`Connection: close`）：客户端照样能用，省得写持久连接。

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 请求头最长多少字节：超过的不答。
const HEAD_LIMIT: usize = 64 * 1024;

/// 跑着的假服务。
pub struct Web {
    /// 端口。
    pub port: u16,
    /// 代理交出去的网页一共几次。
    pages: Arc<Mutex<usize>>,
}

impl Web {
    /// 在 `127.0.0.1` 上随便挑个端口起来。
    ///
    /// # Errors
    ///
    /// 绑不上端口。
    pub async fn start() -> Result<Web, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("假网页服务绑不上端口：{e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("假网页服务的端口读不出：{e}"))?
            .port();
        let pages = Arc::new(Mutex::new(0));
        let counted = Arc::clone(&pages);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, port, Arc::clone(&counted)));
            }
        });
        Ok(Web { port, pages })
    }

    /// 代理交出去的网页一共几次。
    pub fn pages(&self) -> usize {
        *self.pages.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 读到的一个请求。
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    /// `GET`、`POST`……
    pub method: String,
    /// 请求行里的目标：代理请求是整个网址，别的是路径加查询。
    pub target: String,
    /// `Authorization` 的值，没有的是空的。
    pub authorization: String,
    /// 请求体。
    pub body: Vec<u8>,
}

/// 读一个请求：头读到空行，再照 `Content-Length` 读请求体。读不全、太长的是 `None`。
async fn read(socket: &mut TcpStream) -> Option<Request> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    let (head, end) = loop {
        let n = socket.read(&mut buffer).await.ok()?;
        if n == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..n]);
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&data[..end]).into_owned(), end + 4);
        }
        if data.len() > HEAD_LIMIT {
            return None;
        }
    };
    let mut request = parse(&head)?;
    let length = header(&head, "content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0usize);
    let mut body = data[end..].to_vec();
    while body.len() < length {
        let n = socket.read(&mut buffer).await.ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..n]);
    }
    body.truncate(length);
    request.body = body;
    Some(request)
}

/// 读请求行和要用的头（不含请求体）。
pub fn parse(head: &str) -> Option<Request> {
    let mut line = head.lines().next()?.split(' ');
    Some(Request {
        method: line.next()?.to_string(),
        target: line.next()?.to_string(),
        authorization: header(head, "authorization").unwrap_or_default(),
        body: Vec::new(),
    })
}

/// 头里名字是 `name`（不分大小写）的那一项的值。
fn header(head: &str, name: &str) -> Option<String> {
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim().to_string())
}

/// 一个回应：状态、类型、正文、另加的头。
struct Answer {
    status: &'static str,
    kind: &'static str,
    body: Vec<u8>,
    extra: Vec<(&'static str, String)>,
}

fn json_answer(status: &'static str, value: &Value) -> Answer {
    Answer {
        status,
        kind: "application/json",
        body: value.to_string().into_bytes(),
        extra: Vec::new(),
    }
}

fn plain(status: &'static str) -> Answer {
    Answer {
        status,
        kind: "text/plain",
        body: Vec::new(),
        extra: Vec::new(),
    }
}

/// 答一个连接上的一个请求。
async fn serve(mut socket: TcpStream, port: u16, pages: Arc<Mutex<usize>>) {
    let Some(request) = read(&mut socket).await else {
        return;
    };
    let answer = route(&request, port, &pages);
    let mut head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        answer.status,
        answer.kind,
        answer.body.len()
    );
    for (name, value) in &answer.extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let written = async {
        socket.write_all(head.as_bytes()).await?;
        socket.write_all(&answer.body).await?;
        socket.shutdown().await
    }
    .await;
    // 客户端没读完就走了：只是这一个请求没答成，量尺那一头等不到会报出来。
    if let Err(e) = written {
        eprintln!("假网页服务答 {} 时写不出去：{e}", request.target);
    }
}

/// 照请求行定回应（见模块开头）。
fn route(request: &Request, port: u16, pages: &Mutex<usize>) -> Answer {
    let base = format!("http://127.0.0.1:{port}");
    let target = request.target.as_str();
    if let Some(rest) = target.strip_prefix("http://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host.ends_with(".invalid") && !path.ends_with(".png") {
            *pages.lock().unwrap_or_else(PoisonError::into_inner) += 1;
            return Answer {
                status: "200 OK",
                kind: "text/html; charset=utf-8",
                body: page(path).into_bytes(),
                extra: Vec::new(),
            };
        }
        return plain("404 Not Found");
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match (request.method.as_str(), path) {
        ("GET", "/search") => {
            let q = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("q="))
                .unwrap_or_default();
            let results: Vec<Value> = (1..8)
                .map(|k| json!({"title": format!("{q} result {k}"), "url": format!("https://example.org/{k}"),
                    "content": format!("About {q}, number {k}."), "publishedDate": format!("2026-11-0{k}T00:00:00")}))
                .collect();
            json_answer("200 OK", &json!({"query": q, "results": results}))
        }
        ("GET", p) if p.starts_with("/.well-known/oauth-protected-resource") => json_answer(
            "200 OK",
            &json!({"resource": format!("{base}/mcp2"), "authorization_servers": [base], "scopes_supported": ["mcp"]}),
        ),
        ("GET", p)
            if p.starts_with("/.well-known/oauth-authorization-server")
                || p.starts_with("/.well-known/openid-configuration") =>
        {
            json_answer(
                "200 OK",
                &json!({"issuer": base, "authorization_endpoint": format!("{base}/authorize"),
                "token_endpoint": format!("{base}/token"), "registration_endpoint": format!("{base}/register"),
                "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"],
                "code_challenge_methods_supported": ["S256"], "token_endpoint_auth_methods_supported": ["none"],
                "authorization_response_iss_parameter_supported": true}),
            )
        }
        ("GET", "/authorize") => {
            let field = |name: &str| {
                query
                    .split('&')
                    .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                    .unwrap_or_default()
                    .to_string()
            };
            let to = format!(
                "{}?code=code-perf&state={}&iss={}",
                decode(&field("redirect_uri")),
                field("state"),
                encode(&base)
            );
            Answer {
                status: "302 Found",
                kind: "text/plain",
                body: Vec::new(),
                extra: vec![("Location", to)],
            }
        }
        ("POST", "/register") => {
            let meta: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
            json_answer(
                "201 Created",
                &json!({"client_id": "client-perf", "redirect_uris": meta["redirect_uris"],
                "token_endpoint_auth_method": "none"}),
            )
        }
        ("POST", "/token") => json_answer(
            "200 OK",
            &json!({"access_token": "token-perf", "token_type": "Bearer",
            "expires_in": 3600, "refresh_token": "refresh-perf", "scope": "mcp"}),
        ),
        ("POST", "/mcp" | "/mcp2") => {
            if path == "/mcp2" && request.authorization != "Bearer token-perf" {
                let challenge = format!(
                    "Bearer resource_metadata=\"{base}/.well-known/oauth-protected-resource/mcp2\", scope=\"mcp\""
                );
                return Answer {
                    status: "401 Unauthorized",
                    kind: "text/plain",
                    body: Vec::new(),
                    extra: vec![("WWW-Authenticate", challenge)],
                };
            }
            let Ok(message) = serde_json::from_slice::<Value>(&request.body) else {
                return plain("400 Bad Request");
            };
            match super::mcp::answer(&message, false) {
                Some(reply) => {
                    let mut answer = json_answer("200 OK", &reply);
                    answer
                        .extra
                        .push(("Mcp-Session-Id", "session-perf".to_string()));
                    answer
                }
                None => plain("202 Accepted"),
            }
        }
        ("GET", "/mcp" | "/mcp2") => plain("405 Method Not Allowed"),
        ("DELETE", _) => plain("200 OK"),
        _ => plain("404 Not Found"),
    }
}

/// 网址里的 `%XX` 解开（回调的地址是编过的）。
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 编进查询的字：字母、数字和 `-._~` 以外的写成 `%XX`。
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// 一页 HTML：标题照 `path`。
pub fn page(path: &str) -> String {
    let title = format!(
        "Heavy page {}",
        if path.is_empty() { "index" } else { path }
    );
    let rows: String = (0..30)
        .map(|i| format!("<tr><td>row {i}</td><td>{}</td></tr>", i * 7))
        .collect();
    let items: String = (0..20)
        .map(|i| format!("<li>item <code>{i}</code></li>"))
        .collect();
    let filler = "Lorem ipsum dolor sit amet. ".repeat(200);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title><meta property=\"og:title\" content=\"{title}\">\
<meta name=\"description\" content=\"A page about {title}.\"></head><body><nav><a href=\"/\">Home</a> | <a href=\"/b.html\">Next</a></nav>\
<main><article><h1>{title}</h1><p>This page explains <strong>{title}</strong> with <em>some</em> detail and a \
<a href=\"https://example.com/x\">link</a>.</p><h2>Table</h2><table><thead><tr><th>Name</th><th>Value</th></tr></thead>\
<tbody>{rows}</tbody></table><h2>List</h2><ul>{items}</ul><ol><li>first</li><li>second</li></ol><h2>Code</h2>\
<pre><code class=\"language-rust\">fn main() {{ println!(\"hi\"); }}</code></pre><blockquote>A quoted line.</blockquote>\
<img src=\"/logo.png\" alt=\"logo\"><p>{filler}</p></article></main><footer>footer text</footer><script>var x = 1;</script>\
</body></html>"
    )
}

/// 在 `url`（`http://127.0.0.1:<端口>/…`）上发一个 `GET`，有 `Location` 的跳过去再发，至多 `hops` 次：替人在浏览器里打开授权页，
/// 授权页跳回核心在回环上听的回调。交回最后一个回应的状态行。
///
/// # Errors
///
/// 地址不是回环上的 `http`、连不上、写不出去、读不到。
pub async fn follow(url: &str, hops: usize) -> Result<String, String> {
    let mut url = url.to_string();
    for _ in 0..=hops {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| format!("不是 http 的地址：{url}"))?;
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let mut socket = TcpStream::connect(host)
            .await
            .map_err(|e| format!("连不上 {host}：{e}"))?;
        let request = format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        socket
            .write_all(request.as_bytes())
            .await
            .map_err(|e| format!("写不出去：{e}"))?;
        let mut reply = Vec::new();
        socket
            .read_to_end(&mut reply)
            .await
            .map_err(|e| format!("读不到：{e}"))?;
        let text = String::from_utf8_lossy(&reply);
        let status = text.lines().next().unwrap_or_default().to_string();
        match header(&text, "location") {
            Some(to) => url = to,
            None => return Ok(status),
        }
    }
    Err(format!("跳了 {hops} 次还在跳：{url}"))
}

#[cfg(test)]
mod tests;
