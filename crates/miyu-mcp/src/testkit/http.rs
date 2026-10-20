//! 假的网址服务（施工 X-32 上）：本机一个端口上演 Streamable HTTP MCP 服务，测试照 [`HttpServer::received`] 查客户端发了
//! 什么。一件工具 `echo` 照参数 `text` 回文字，通知回 `202`。
//!
//! - 旧时代的（[`HttpServer::start`]）：`server/discover` 回「没有这个方法」，`initialize` 给会话编号 `s-1`。
//! - 新时代的（[`HttpServer::modern`]，施工 X-32 补）：照 GitHub 的远程服务，请求 `_meta` 里写了版本、`MCP-Protocol-Version`
//!   头里没有同一个的回 `400`（正文是 `-32020` 的 JSON-RPC 错）；对得上的照新时代答，不认 `initialize`。
//!
//! 每个回应都带 `Connection: close`：一个请求一条连接，不用演长连接。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// 收到的一个请求：头（名字小写）和正文。
#[derive(Debug, Clone)]
pub struct Received {
    /// 请求头，名字都写成小写。
    pub headers: Vec<(String, String)>,
    /// 正文，读成 JSON。
    pub body: Value,
}

impl Received {
    /// 名字是 `name`（小写）的头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// 一个在跑的假服务；丢掉就停。
pub struct HttpServer {
    /// 它的网址：`http://127.0.0.1:<端口>/mcp`。
    pub url: String,
    state: Arc<State>,
    task: tokio::task::JoinHandle<()>,
}

struct State {
    sse: bool,
    /// 演新时代的（施工 X-32 补）。
    modern: bool,
    received: Mutex<Vec<Received>>,
    refuse: AtomicU16,
    /// 要的令牌（施工 X-32 补）：有的话，请求不带 `Authorization: Bearer <它>` 的回 `401`。
    demand: Mutex<Option<String>>,
    expire: AtomicBool,
    oversize: AtomicBool,
}

impl HttpServer {
    /// 起一个：`sse` 的回应写成 SSE，不然是 JSON。
    ///
    /// # Panics
    ///
    /// 本机的端口开不了。
    pub async fn start(sse: bool) -> HttpServer {
        HttpServer::open(sse, false).await
    }

    /// 起一个演新时代的（施工 X-32 补）：版本写在每个请求的 `_meta` 和 `MCP-Protocol-Version` 头里，两处对不上的回 `400`。
    ///
    /// # Panics
    ///
    /// 本机的端口开不了。
    pub async fn modern(sse: bool) -> HttpServer {
        HttpServer::open(sse, true).await
    }

    async fn open(sse: bool, modern: bool) -> HttpServer {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("本机的端口开得了");
        let url = format!("http://{}/mcp", listener.local_addr().expect("有地址"));
        let state = Arc::new(State {
            sse,
            modern,
            received: Mutex::new(Vec::new()),
            refuse: AtomicU16::new(0),
            demand: Mutex::new(None),
            expire: AtomicBool::new(false),
            oversize: AtomicBool::new(false),
        });
        let serving = Arc::clone(&state);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(answer(stream, Arc::clone(&serving)));
            }
        });
        HttpServer { url, state, task }
    }

    /// 收到的请求，照先后。
    pub fn received(&self) -> Vec<Received> {
        self.state
            .received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 之后的请求一律回 HTTP `status`（`0` 是照常答）。
    pub fn refuse(&self, status: u16) {
        self.state.refuse.store(status, Ordering::Relaxed);
    }

    /// 之后的请求要带 `Authorization: Bearer <token>`，没带对的回 `401`；`None` 是不查（施工 X-32 补）。
    pub fn demand(&self, token: Option<&str>) {
        *self
            .state
            .demand
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = token.map(str::to_string);
    }

    /// 下一个带会话编号的请求回 `404`（会话过期），之后照常。
    pub fn expire(&self) {
        self.state.expire.store(true, Ordering::Relaxed);
    }

    /// 下一个调工具的请求回一段比管道一行的上限还长一个字节的正文（施工 X-32 下），之后照常。握手以后的通知可能晚到，不算。
    pub fn oversize(&self) {
        self.state.oversize.store(true, Ordering::Relaxed);
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 答一条连接上的一个请求。
async fn answer(stream: TcpStream, state: Arc<State>) {
    let mut reader = BufReader::new(stream);
    let mut headers = Vec::new();
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_err() {
        return;
    }
    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.trim_end().split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).await.is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let request = Received { headers, body };
    state
        .received
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request.clone());
    let (status, extra, text) = reply(&state, &request);
    let kind = if state.sse {
        "text/event-stream"
    } else {
        "application/json"
    };
    let mut head = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        text.len()
    );
    if !text.is_empty() {
        head.push_str(&format!("Content-Type: {kind}\r\n"));
    }
    head.push_str(&extra);
    head.push_str("\r\n");
    let mut stream = reader.into_inner();
    if stream.write_all(head.as_bytes()).await.is_ok()
        && stream.write_all(text.as_bytes()).await.is_ok()
    {
        drop(stream.shutdown().await);
    }
}

/// 照请求答：状态、另加的头、正文。
fn reply(state: &State, request: &Received) -> (u16, String, String) {
    let demand = state
        .demand
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(token) = demand
        && request.header("authorization") != Some(format!("Bearer {token}").as_str())
    {
        return (401, String::new(), "bad credentials".to_string());
    }
    let refuse = state.refuse.load(Ordering::Relaxed);
    if refuse != 0 {
        return (refuse, String::new(), "nope".to_string());
    }
    if request.header("mcp-session-id").is_some() && state.expire.swap(false, Ordering::Relaxed) {
        return (404, String::new(), String::new());
    }
    if request.body["method"] == "tools/call" && state.oversize.swap(false, Ordering::Relaxed) {
        return (200, String::new(), " ".repeat(crate::wire::LINE_LIMIT + 1));
    }
    let message = &request.body;
    let Some(id) = message.get("id") else {
        return (202, String::new(), String::new());
    };
    if state.modern {
        return modern(state, request, id);
    }
    let (result, extra) = match message["method"].as_str() {
        Some("initialize") => (
            Ok(
                json!({"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "1"}}),
            ),
            "Mcp-Session-Id: s-1\r\n".to_string(),
        ),
        Some("tools/list") => (
            Ok(
                json!({"tools": [{"name": "echo", "description": "Say it back.", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}),
            ),
            String::new(),
        ),
        Some("tools/call") if message["params"]["name"] == "echo" => (
            Ok(
                json!({"content": [{"type": "text", "text": message["params"]["arguments"]["text"]}]}),
            ),
            String::new(),
        ),
        _ => (
            Err(json!({"code": -32601, "message": "method not found"})),
            String::new(),
        ),
    };
    let answer = match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    };
    let text = if state.sse {
        format!("event: message\ndata: {answer}\n\n")
    } else {
        answer.to_string()
    };
    (200, extra, text)
}

/// 新时代的答法（施工 X-32 补）：`_meta` 里的版本要和 `MCP-Protocol-Version` 头一样，不然照 GitHub 回 `400`；认 `server/discover`、
/// `tools/list`、`tools/call`，结果带 `resultType`；`initialize` 回「没有这个方法」。
fn modern(state: &State, request: &Received, id: &Value) -> (u16, String, String) {
    let message = &request.body;
    let asked = message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
    if asked.is_some() && request.header("mcp-protocol-version") != asked {
        let refused = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32020, "message":
            "Mcp-Protocol-Version header is required for requests carrying \"io.modelcontextprotocol/protocolVersion\""}});
        return (400, String::new(), refused.to_string());
    }
    let result = match message["method"].as_str() {
        Some("server/discover") => Ok(
            json!({"resultType": "complete", "supportedVersions": crate::MODERN,
            "capabilities": {"tools": {}}, "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "fake", "version": "1"}}}),
        ),
        Some("tools/list") => Ok(
            json!({"resultType": "complete", "tools": [{"name": "echo", "description": "Say it back.",
            "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}),
        ),
        Some("tools/call") if message["params"]["name"] == "echo" => {
            Ok(json!({"resultType": "complete",
            "content": [{"type": "text", "text": message["params"]["arguments"]["text"]}]}))
        }
        _ => Err(json!({"code": -32601, "message": "method not found"})),
    };
    let answer = match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    };
    let text = if state.sse {
        format!("event: message\ndata: {answer}\n\n")
    } else {
        answer.to_string()
    };
    (200, String::new(), text)
}
