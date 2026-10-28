//! 网址的服务（施工 X-32 上，`docs/blueprint/mcp.md`「网址的服务怎么说话」）：把一行一条的 JSON-RPC 接到 Streamable HTTP 上，
//! 交回一对读写，[`crate::Client::connect`] 照常用，认时代、列工具、调工具、取消都不用改。
//!
//! - 客户端写出的每一行 POST 到网址，几条同时在路上的各发各的；`Accept` 两种都要，带上服务给过的 `Mcp-Session-Id`、协议版本
//!   （`MCP-Protocol-Version`），和清单写的请求头。版本先照这一条自己 `_meta` 里写的（新时代，施工 X-32 补：GitHub 的远程服务
//!   没有它就回 `400`），没写的照握手认出来的（老时代），握手那一次还没有，不带。
//! - 规范要的标准请求头（施工 PT-1 上，规范 2026-07-28「Standard Request Headers」）：每个请求带 `Mcp-Method`，`tools/call`、
//!   `resources/read`、`prompts/get` 另带 `Mcp-Name`（`params.name` 或 `params.uri`）；不是可见 ASCII、首尾有空白、长得像编码
//!   记号的照 `=?base64?…?=` 编。新版服务缺了这两个回 `400`；老服务不认这两个头，照样带着无害，不分时代。
//! - 工具参数镜像成请求头（施工 PT-1 中，`http/params.rs`）：列工具的回应经过这里时照规范查每件工具的 `x-mcp-header`，不合的从列表里
//!   剔掉、往日志写一行；合的记下镜像哪几个参数，调这件工具时带上 `Mcp-Param-<名>`。
//! - 取消（施工 PT-1 中）：客户端发 `notifications/cancelled` 时，停掉那个请求还在路上的 POST（连接跟着断，新版规范里断开回应流
//!   就是取消）；老版本的服务（握过手的）另把这条通知照旧发过去，新版的不发。
//! - 回来的是 SSE 的边读边写回去（施工 PT-1 下，`http/answer.rs`）：凑齐一个事件就写，进度、订阅推来的通知当场到。
//! - 回来的是 JSON 的照一行写回去，是 SSE 的照 `data:` 一条一条写回去；`202` 的（通知、回应）什么都不写。JSON 的整段读完再写，
//!   一段至多读 64 MiB（同管道一行的上限，施工 X-32 下），再长的当对方回错；SSE 的边读边写，一个事件的上限同它。
//! - 对方回错的（4xx、5xx、连不上）：有编号的请求回一条 JSON-RPC 错误，客户端照常了结这次请求；往 `log` 写一行。日志里不写网址：
//!   有的服务把密钥放在网址的查询串里。回应体本身是一条 JSON-RPC 错误的（新版服务的 `400` 都这样：版本不认 `-32022`、请求头
//!   不对 `-32020`，施工 PT-1 上）原样交回、编号换成这次请求的，探时代的那一头才认得出是新版的服务；不是的才合成一条。
//! - 要登录（回 `401`，施工 X-42 上）：`WWW-Authenticate` 的原文记下来（[`Opened::challenged`]），看管的一头照它判要不要登录；
//!   回给客户端的照对方回错一样。
//! - 登录来的服务（施工 X-42 下，[`Credentials`]）：每个请求现场问一次令牌；回 `401` 的续一次、照新令牌把这一次请求重发一遍，再
//!   `401` 的、续不成的记下挑战、关掉这一对读写，看管的一头停在 `needs_login`。回 `403`、`error="insufficient_scope"` 的同样记下、
//!   关掉（规范的 step-up）。写了 `bearer`、`headers` 的服务没有凭据，照旧：回错只了结那一次请求。
//! - 会话过期（带着会话编号回 `404`）：关掉这一对读写，客户端当服务断了，看管的那一头退避着重连、从认时代重来。
//! - 服务主动推的只在 SSE 回应里收得到，不另开 GET 那条长连接：新时代的「变了」走客户端开的 `subscriptions/listen`，它的回应就是
//!   一条一直开着的 SSE（施工 PT-1 下）；老时代的照旧靠 `ttlMs` 和重连时重列。

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use miyu_http::Client;

mod answer;
mod headers;
mod params;

use answer::{Stream, body, rpc_error};
use headers::{header_value, standard_headers};

/// 内存管道的缓冲：够放几条大的回应，放不下的写的一头等着读。
const PIPE: usize = 1 << 20;

/// 对方回错时写给客户端的 JSON-RPC 错误码（实现自定的范围）。
const HTTP_ERROR: i64 = -32000;

/// 往服务的日志里写一行（核心照服务名写进日志目录里它的一份）。
pub type Log = Arc<dyn Fn(String) + Send + Sync>;

/// 等一个令牌：没有的是空的。
pub type Pending<'a> = Pin<Box<dyn Future<Output = Option<String>> + Send + 'a>>;

/// 每个请求现场给的令牌（施工 X-42 下）：登录来的服务由看管的一头给，快过期的先续再给。
pub trait Credentials: Send + Sync {
    /// 这一次请求带的令牌：没有的不带 `Authorization`。
    fn token(&self) -> Pending<'_>;

    /// 带着 `rejected` 被回了 `401`：续一次，交回新的；续不成的没有。几个请求同时撞上的只续一次（续过了的直接交回现在的）。
    fn renew<'a>(&'a self, rejected: &'a str) -> Pending<'a>;
}

/// 连上的一个网址：给 [`crate::Client::connect`] 的读、写两头（丢了，转接跟着停），和这条连接断了是不是因为会话过期。
pub struct Opened {
    /// 读的一头。
    pub read: ReadHalf<DuplexStream>,
    /// 写的一头。
    pub write: WriteHalf<DuplexStream>,
    /// 会话过期断的：看管的一头照它重连，不当一次失败（`mcp.md`「网址的服务怎么说话」第 3 条）。
    pub expired: Expired,
    /// 回过 `401` 的：它的 `WWW-Authenticate`（施工 X-42 上）。
    pub challenged: Challenged,
}

/// 这条连接是不是因为会话过期断的。
#[derive(Debug, Clone, Default)]
pub struct Expired(Arc<AtomicBool>);

/// 对方回过 `401`：最近那一次的 `WWW-Authenticate` 原文（施工 X-42 上）。
#[derive(Debug, Clone, Default)]
pub struct Challenged(Arc<Mutex<Option<String>>>);

impl Challenged {
    /// 最近那一次 `401` 的 `WWW-Authenticate`，没回过的没有。
    pub fn header(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Expired {
    /// 会话过期了，转接关了这一对读写。
    pub fn happened(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 连网址 `url`：`headers` 是每个请求都带的（写了的令牌已经换成 `Authorization` 了），`log` 收对方回错这一类，`credentials` 是登录来的
/// 服务现场给令牌的（施工 X-42 下），别的没有。
pub fn open(
    client: Client,
    url: String,
    headers: Vec<(String, String)>,
    log: Log,
    credentials: Option<Arc<dyn Credentials>>,
) -> Opened {
    let (theirs, ours) = tokio::io::duplex(PIPE);
    let (read, write) = tokio::io::split(theirs);
    let expired = Expired::default();
    let challenged = Challenged::default();
    tokio::spawn(relay(
        ours,
        Arc::new(Remote {
            client,
            url,
            headers,
            log,
            credentials,
            session: Mutex::new(None),
            version: Mutex::new(None),
            params: Mutex::new(HashMap::new()),
            expired: expired.clone(),
            challenged: challenged.clone(),
        }),
    ));
    Opened {
        read,
        write,
        expired,
        challenged,
    }
}

/// 一个网址的服务：发请求要的，和对方给过的会话编号、认出来的协议版本。
struct Remote {
    client: Client,
    url: String,
    headers: Vec<(String, String)>,
    log: Log,
    credentials: Option<Arc<dyn Credentials>>,
    session: Mutex<Option<String>>,
    version: Mutex<Option<String>>,
    /// 列工具时记下的：工具名到镜像成请求头的参数（施工 PT-1 中）。
    params: Mutex<HashMap<String, Vec<params::Param>>>,
    expired: Expired,
    challenged: Challenged,
}

/// 一条 POST 的结局：写回去的几行；会话过期的、登录不再算数的（施工 X-42 下）要关掉这一对读写。
enum Answered {
    Lines(Vec<String>),
    Expired,
    Closed,
}

/// 读客户端写出的一行行，各发一个 POST；回来的经一个写的任务写回去。读到头（客户端丢了）、会话过期就停，在路上的跟着停。
async fn relay(ours: DuplexStream, remote: Arc<Remote>) {
    let (read, mut write) = tokio::io::split(ours);
    let (answers, mut answered) = mpsc::unbounded_channel::<Answered>();
    let mut writer = tokio::spawn(async move {
        while let Some(answer) = answered.recv().await {
            let Answered::Lines(lines) = answer else {
                return;
            };
            for line in lines {
                if write.write_all(line.as_bytes()).await.is_err()
                    || write.write_all(b"\n").await.is_err()
                {
                    return;
                }
            }
            if write.flush().await.is_err() {
                return;
            }
        }
    });
    let mut lines = BufReader::new(read).lines();
    let mut sending = JoinSet::new();
    // 还在路上的请求：编号（JSON 原文）到它的 POST，取消时照它停（施工 PT-1 中）。
    let mut in_flight: HashMap<String, tokio::task::AbortHandle> = HashMap::new();
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) if !line.trim().is_empty() => {
                    let message = serde_json::from_str::<Value>(&line).ok();
                    if let Some(cancelled) = message.as_ref().and_then(cancelled) {
                        if let Some(task) = in_flight.remove(&cancelled) {
                            task.abort();
                        }
                        // 新版服务：断开就是取消，不另发通知；握过手的老服务照旧发。
                        if remote.version.lock().unwrap_or_else(PoisonError::into_inner).is_none() {
                            continue;
                        }
                    }
                    let id = message.as_ref().and_then(|message| message.get("id")).map(Value::to_string);
                    let (remote, answers) = (Arc::clone(&remote), answers.clone());
                    let task = sending.spawn(async move {
                        let last = remote.post(&line, &answers).await;
                        if answers.send(last).is_err() {
                            // 写的一头已经停了。
                        }
                    });
                    if let Some(id) = id {
                        in_flight.insert(id, task);
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            },
            // 写的任务停了（会话过期、客户端那一头不读了）：等它停的通知，不去轮询。
            _ = &mut writer => break,
        }
        while sending.try_join_next().is_some() {}
        in_flight.retain(|_, task| !task.is_finished());
    }
    sending.abort_all();
    writer.abort();
}

impl Remote {
    /// 发一行，交回要写回去的；SSE 回应边读边写的，读的时候一段段先经 `answers` 写回去，交回最后剩下的（施工 PT-1 下）。
    async fn post(&self, line: &str, answers: &mpsc::UnboundedSender<Answered>) -> Answered {
        let message = serde_json::from_str::<Value>(line).ok();
        let id = message
            .as_ref()
            .and_then(|message| message.get("id").cloned());
        // 新时代的请求自己带着版本（施工 X-32 补）。
        let asked = message.as_ref().and_then(|message| {
            message["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"]
                .as_str()
                .map(str::to_string)
        });
        let session = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let version = asked.or_else(|| {
            self.version
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        });
        let token = match &self.credentials {
            Some(credentials) => credentials.token().await,
            None => None,
        };
        let mut standard = message.as_ref().map(standard_headers).unwrap_or_default();
        if let Some(message) = &message {
            standard.extend(self.param_headers(message));
        }
        let sent = Sent {
            session: session.as_deref(),
            version: version.as_deref(),
            standard: &standard,
        };
        let mut response = match self.send(line, &sent, token.as_deref()).await {
            Ok(response) => response,
            Err(why) => return self.failed(id, &why),
        };
        // 登录来的服务回 `401`：续一次，照新令牌把这一次重发（施工 X-42 下）。
        if response.status().as_u16() == 401
            && let (Some(credentials), Some(rejected)) = (&self.credentials, &token)
            && let Some(renewed) = credentials.renew(rejected).await
        {
            response = match self.send(line, &sent, Some(&renewed)).await {
                Ok(response) => response,
                Err(why) => return self.failed(id, &why),
            };
        }
        let status = response.status().as_u16();
        let header = response
            .headers()
            .get("www-authenticate")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let more_scope = status == 403
            && header
                .as_deref()
                .and_then(crate::oauth::challenge)
                .is_some_and(|challenge| challenge.error.as_deref() == Some("insufficient_scope"));
        if (status == 401 || more_scope)
            && let Some(header) = header
        {
            *self
                .challenged
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(header);
            // 登录来的服务：关掉这一对读写，看管的一头照挑战停在 `needs_login`（施工 X-42 下）。
            if self.credentials.is_some() {
                (self.log)(format!("HTTP {status}: the login is no longer accepted"));
                return Answered::Closed;
            }
        }
        if status == 404 && session.is_some() {
            (self.log)("session expired".to_string());
            self.expired.0.store(true, Ordering::Relaxed);
            return Answered::Expired;
        }
        if let Some(given) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
        {
            *self.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(given.to_string());
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|kind| kind.starts_with("text/event-stream"));
        let listing = message
            .as_ref()
            .is_some_and(|message| message["method"] == "tools/list");
        if sse && (200..300).contains(&status) {
            return self.stream(response, (id, listing), answers).await;
        }
        let body = match body(response).await {
            Ok(body) => body,
            Err(why) => return self.failed(id, &why),
        };
        if !(200..300).contains(&status) {
            if let Some(error) = rpc_error(&body, sse, id.as_ref()) {
                (self.log)(format!("HTTP {status}: {}", error.1));
                return Answered::Lines(error.0.into_iter().collect());
            }
            return self.failed(
                id,
                &format!(
                    "HTTP {status}: {}",
                    body.chars().take(200).collect::<String>()
                ),
            );
        }
        // 走到这里的是 JSON：SSE 的成功回应上面边读边拆了。
        let messages = body.trim().lines().map(str::to_string).collect();
        Answered::Lines(self.handled(messages, listing))
    }

    /// 边读边拆一段 SSE 回应（施工 PT-1 下）：凑齐的事件经 `answers` 马上写回去，交回读完以后剩下的。读断了、太长的照对方回错
    /// （`id` 是这次请求的编号）；`listing` 的照列工具那样查。
    async fn stream(
        &self,
        mut response: miyu_http::Response,
        (id, listing): (Option<Value>, bool),
        answers: &mpsc::UnboundedSender<Answered>,
    ) -> Answered {
        let mut stream = Stream::default();
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(error) => {
                    let why = format!("the answer broke off: {}", error.without_url());
                    return self.failed(id, &why);
                }
            };
            match stream.push(&chunk) {
                Ok(events) if events.is_empty() => {}
                Ok(events) => {
                    if answers
                        .send(Answered::Lines(self.handled(events, listing)))
                        .is_err()
                    {
                        // 写的一头已经停了。
                        return Answered::Lines(Vec::new());
                    }
                }
                Err(why) => return self.failed(id, &why),
            }
        }
        Answered::Lines(self.handled(stream.finish(), listing))
    }

    /// 回来的几条：握手的记下版本，列工具的照规范查（[`Remote::screen`]）。
    fn handled(&self, messages: Vec<String>, listing: bool) -> Vec<String> {
        for message in &messages {
            self.notice_version(message);
        }
        if listing {
            return messages.iter().map(|line| self.screen(line)).collect();
        }
        messages
    }

    /// 发一次 POST：带上会话编号、协议版本、规范的标准请求头（`sent`）、清单写的请求头，有令牌 `token` 的带 `Authorization`。连不上
    /// 的交回原因。
    async fn send(
        &self,
        line: &str,
        sent: &Sent<'_>,
        token: Option<&str>,
    ) -> Result<miyu_http::Response, String> {
        let mut request = self
            .client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(line.to_string());
        if let Some(session) = sent.session {
            request = request.header("Mcp-Session-Id", session);
        }
        if let Some(version) = sent.version {
            request = request.header("MCP-Protocol-Version", version);
        }
        for (name, value) in sent.standard {
            request = request.header(name.as_str(), value);
        }
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        if let Some(token) = token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        request
            .send()
            .await
            .map_err(|error| format!("cannot reach the server: {}", error.without_url()))
    }

    /// 对方回错：记一行；有编号的回一条错误，客户端了结这次请求。
    fn failed(&self, id: Option<Value>, why: &str) -> Answered {
        (self.log)(why.to_string());
        match id {
            Some(id) => Answered::Lines(vec![
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": HTTP_ERROR, "message": why}})
                    .to_string(),
            ]),
            None => Answered::Lines(Vec::new()),
        }
    }

    /// 握手的回应里有协议版本的记下，以后每个请求带上它（Streamable HTTP 要求）。
    fn notice_version(&self, message: &str) {
        let version = serde_json::from_str::<Value>(message)
            .ok()
            .and_then(|message| {
                message["result"]["protocolVersion"]
                    .as_str()
                    .map(str::to_string)
            });
        if let Some(version) = version {
            *self.version.lock().unwrap_or_else(PoisonError::into_inner) = Some(version);
        }
    }
}

/// 一次 POST 除了正文要带的：服务给过的会话编号、协议版本、规范的标准请求头（[`standard_headers`]）。
struct Sent<'a> {
    session: Option<&'a str>,
    version: Option<&'a str>,
    standard: &'a [(String, String)],
}

/// 客户端发的这一条是取消通知的：被取消的请求的编号（JSON 原文，同 `in_flight` 的键）。
fn cancelled(message: &Value) -> Option<String> {
    (message["method"] == "notifications/cancelled")
        .then(|| message["params"].get("requestId").map(Value::to_string))
        .flatten()
}

#[cfg(test)]
mod tests;
