//! 网址的服务（施工 X-32 上，`docs/blueprint/mcp.md`「网址的服务怎么说话」）：把一行一条的 JSON-RPC 接到 Streamable HTTP 上，
//! 交回一对读写，[`crate::Client::connect`] 照常用，认时代、列工具、调工具、取消都不用改。
//!
//! - 客户端写出的每一行 POST 到网址，几条同时在路上的各发各的；`Accept` 两种都要，带上服务给过的 `Mcp-Session-Id`、认出来的协议
//!   版本（`MCP-Protocol-Version`），和清单写的请求头。
//! - 回来的是 JSON 的照一行写回去，是 SSE 的照 `data:` 一条一条写回去；`202` 的（通知、回应）什么都不写。整段读完再写：一次
//!   调用的回应本来就要等它说完，不另开流式读。一段回应至多读 64 MiB（同管道一行的上限，施工 X-32 下），再长的当对方回错。
//! - 对方回错的（4xx、5xx、连不上）：有编号的请求回一条 JSON-RPC 错误，客户端照常了结这次请求；往 `log` 写一行。日志里不写网址：
//!   有的服务把密钥放在网址的查询串里。
//! - 会话过期（带着会话编号回 `404`）：关掉这一对读写，客户端当服务断了，看管的那一头退避着重连、从认时代重来。
//! - 服务主动推的只在 SSE 回应里收得到，不另开 GET 那条长连接（工具变了靠新时代的 `ttlMs`、重连时重列）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use miyu_http::Client;

use crate::wire::LINE_LIMIT;

/// 内存管道的缓冲：够放几条大的回应，放不下的写的一头等着读。
const PIPE: usize = 1 << 20;

/// 对方回错时写给客户端的 JSON-RPC 错误码（实现自定的范围）。
const HTTP_ERROR: i64 = -32000;

/// 往服务的日志里写一行（核心照服务名写进日志目录里它的一份）。
pub type Log = Arc<dyn Fn(String) + Send + Sync>;

/// 连上的一个网址：给 [`crate::Client::connect`] 的读、写两头（丢了，转接跟着停），和这条连接断了是不是因为会话过期。
pub struct Opened {
    /// 读的一头。
    pub read: ReadHalf<DuplexStream>,
    /// 写的一头。
    pub write: WriteHalf<DuplexStream>,
    /// 会话过期断的：看管的一头照它重连，不当一次失败（`mcp.md`「网址的服务怎么说话」第 3 条）。
    pub expired: Expired,
}

/// 这条连接是不是因为会话过期断的。
#[derive(Debug, Clone, Default)]
pub struct Expired(Arc<AtomicBool>);

impl Expired {
    /// 会话过期了，转接关了这一对读写。
    pub fn happened(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// 连网址 `url`：`headers` 是每个请求都带的（令牌已经换成 `Authorization` 了），`log` 收对方回错这一类。
pub fn open(client: Client, url: String, headers: Vec<(String, String)>, log: Log) -> Opened {
    let (theirs, ours) = tokio::io::duplex(PIPE);
    let (read, write) = tokio::io::split(theirs);
    let expired = Expired::default();
    tokio::spawn(relay(
        ours,
        Arc::new(Remote {
            client,
            url,
            headers,
            log,
            session: Mutex::new(None),
            version: Mutex::new(None),
            expired: expired.clone(),
        }),
    ));
    Opened {
        read,
        write,
        expired,
    }
}

/// 一个网址的服务：发请求要的，和对方给过的会话编号、认出来的协议版本。
struct Remote {
    client: Client,
    url: String,
    headers: Vec<(String, String)>,
    log: Log,
    session: Mutex<Option<String>>,
    version: Mutex<Option<String>>,
    expired: Expired,
}

/// 一条 POST 的结局：写回去的几行；会话过期的要关掉这一对读写。
enum Answered {
    Lines(Vec<String>),
    Expired,
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
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) if !line.trim().is_empty() => {
                    let (remote, answers) = (Arc::clone(&remote), answers.clone());
                    sending.spawn(async move {
                        if answers.send(remote.post(&line).await).is_err() {
                            // 写的一头已经停了。
                        }
                    });
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            },
            // 写的任务停了（会话过期、客户端那一头不读了）：等它停的通知，不去轮询。
            _ = &mut writer => break,
        }
        while sending.try_join_next().is_some() {}
    }
    sending.abort_all();
    writer.abort();
}

impl Remote {
    /// 发一行，交回要写回去的。
    async fn post(&self, line: &str) -> Answered {
        let id = serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|message| message.get("id").cloned());
        let mut request = self
            .client
            .post(&self.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(line.to_string());
        let session = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(session) = &session {
            request = request.header("Mcp-Session-Id", session);
        }
        if let Some(version) = self
            .version
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            request = request.header("MCP-Protocol-Version", version);
        }
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return self.failed(
                    id,
                    &format!("cannot reach the server: {}", error.without_url()),
                );
            }
        };
        let status = response.status().as_u16();
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
        let body = match body(response).await {
            Ok(body) => body,
            Err(why) => return self.failed(id, &why),
        };
        if !(200..300).contains(&status) {
            return self.failed(
                id,
                &format!(
                    "HTTP {status}: {}",
                    body.chars().take(200).collect::<String>()
                ),
            );
        }
        let messages = if sse {
            events(&body)
        } else {
            body.trim().lines().map(str::to_string).collect()
        };
        for message in &messages {
            self.notice_version(message);
        }
        Answered::Lines(messages)
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

/// 读完一段回应：读断了的、超过 [`LINE_LIMIT`] 的交回原因（再长的一条客户端也收不下，不往内存里一直读）。
async fn body(mut response: miyu_http::Response) -> Result<String, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("the answer broke off: {}", error.without_url()))?
    {
        if bytes.len() + chunk.len() > LINE_LIMIT {
            return Err(format!("the answer is longer than {LINE_LIMIT} bytes"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// SSE 的正文拆成一条条消息：空行分开一个事件，一个事件里的几行 `data:` 接起来；`\r\n` 也算换行。
fn events(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut data: Vec<&str> = Vec::new();
    for line in body.lines().chain(std::iter::once("")) {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if !data.is_empty() {
                out.push(data.join("\n"));
                data.clear();
            }
        } else if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    out
}

#[cfg(test)]
mod tests;
