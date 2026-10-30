//! 一条连接（施工 X-1）：写的一头一个任务、读的一头一个任务，请求照编号等回应。读到头、读到太长的一行、写不进去，这条连接就
//! 断了：在等的都收到 [`Failed::Closed`]，之后发的当场收到。
//!
//! 服务反过来的请求在读的那个任务里当场回：`ping` 回空的，别的回「没有这个方法」（`-32601`）。工具变了的通知数一次。进度通知
//! （`notifications/progress`，施工 PT-1 中下）照 `progressToken` 交给跟着它的那一头（[`Link::follow`]），没人跟的丢掉。
//!
//! 写的队列、每个进度令牌的通知队列最多排 [`QUEUE`] 条（施工 AE-8）：服务不读标准输入、跟进度的那一头不收，内存不跟着涨。
//! 满了的通知（核心发的、服务发来的进度）丢掉，记一行 `DEBUG`；回服务的 `ping` 也一样；请求等空位，等的时间算在等回应的
//! 时限里。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot, watch};

use crate::TARGET;
use crate::error::Failed;
use crate::wire::{self, Incoming, Read};

/// 「没有这个方法」。
const METHOD_NOT_FOUND: i64 = -32601;

/// 写的队列、一个进度令牌的通知队列最多排几条（施工 AE-8）：同核心给每条连接的出站队列（`miyu-endpoint` 的
/// `connection::QUEUE`）。
pub(crate) const QUEUE: usize = 256;

/// 一条连接。
pub(crate) struct Link {
    out: mpsc::Sender<Vec<u8>>,
    shared: Arc<Shared>,
    next: AtomicU64,
}

/// 读写两个任务和发请求的一方共用的。
struct Shared {
    waiting: Mutex<Waiting>,
    /// 服务说工具变了几次。
    changes: watch::Sender<u64>,
    /// 断了没有。
    closed: watch::Sender<bool>,
}

/// 在等回应的请求，和跟着进度的。
#[derive(Default)]
struct Waiting {
    open: bool,
    asked: BTreeMap<u64, oneshot::Sender<Result<Value, Value>>>,
    /// 进度令牌（JSON 原文）到收进度通知的一头（施工 PT-1 下）。
    following: BTreeMap<String, mpsc::Sender<Value>>,
}

impl Link {
    /// 照服务的标准输出 `reader`、标准输入 `writer` 起读写两个任务。
    pub(crate) fn start<R, W>(reader: R, writer: W) -> Link
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (out, lines) = mpsc::channel(QUEUE);
        let shared = Arc::new(Shared {
            waiting: Mutex::new(Waiting {
                open: true,
                ..Waiting::default()
            }),
            changes: watch::channel(0).0,
            closed: watch::channel(false).0,
        });
        tokio::spawn(write(writer, lines, Arc::clone(&shared)));
        // 读的一头只拿弱的：连接丢下了，写的一头随之收尾、关掉服务的标准输入（施工 X-3 上：核心停服务先关它的标准输入）。
        tokio::spawn(read(reader, out.downgrade(), Arc::clone(&shared)));
        Link {
            out,
            shared,
            next: AtomicU64::new(1),
        }
    }

    /// 发请求 `method`，等回应：`wait` 是最多等多久，没写的一直等。`cancel` 的，没等到回应就不等了（超时、这个 future 被丢掉）
    /// 时发 `notifications/cancelled`；握手那几个不发（MCP 规范：`initialize` 不许取消）。
    pub(crate) async fn ask(
        &self,
        method: &str,
        params: Value,
        wait: Option<Duration>,
        cancel: bool,
    ) -> Result<Value, Failed> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = oneshot::channel();
        {
            let mut waiting = self.shared.lock();
            if !waiting.open {
                return Err(Failed::Closed);
            }
            waiting.asked.insert(id, reply);
        }
        let mut asking = Asking {
            link: self,
            id,
            cancel,
            done: false,
        };
        // 排空位和等回应算同一个时限：服务不读标准输入、队列满着的，到点照没等到回应收（施工 AE-8）。
        let sent_and_answered = async {
            if self
                .out
                .send(wire::request(id, method, params))
                .await
                .is_err()
            {
                return Err(Failed::Closed);
            }
            Ok(answer.await)
        };
        let answered = match wait {
            Some(wait) => tokio::time::timeout(wait, sent_and_answered)
                .await
                .map_err(|_| Failed::TimedOut)??,
            None => sent_and_answered.await?,
        };
        asking.done = true;
        match answered {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(rpc(error)),
            Err(_) => Err(Failed::Closed),
        }
    }

    /// 发通知 `method`。断了的、写的队列满了的不发（施工 AE-8）。
    pub(crate) fn tell(&self, method: &str, params: Value) {
        if let Err(error) = self.out.try_send(wire::notice(method, params)) {
            tracing::debug!(target: TARGET, method, error = %error, "notice not sent");
        }
    }

    /// 跟着进度令牌 `token` 的进度通知（施工 PT-1 下）：交回收通知参数的一头；丢掉 [`Following`] 就不跟了。
    pub(crate) fn follow(&self, token: &Value) -> (mpsc::Receiver<Value>, Following<'_>) {
        let (sender, updates) = mpsc::channel(QUEUE);
        let key = token.to_string();
        self.shared.lock().following.insert(key.clone(), sender);
        (updates, Following { link: self, key })
    }

    /// 服务说工具变了几次。
    pub(crate) fn changes(&self) -> watch::Receiver<u64> {
        self.shared.changes.subscribe()
    }

    /// 等到这条连接断了。
    pub(crate) async fn closed(&self) {
        let mut closed = self.shared.closed.subscribe();
        if closed.wait_for(|closed| *closed).await.is_err() {
            tracing::debug!(target: TARGET, "close flag dropped");
        }
    }
}

/// 跟着一个进度令牌：丢掉就不跟了。
pub(crate) struct Following<'a> {
    link: &'a Link,
    key: String,
}

impl Drop for Following<'_> {
    fn drop(&mut self) {
        self.link.shared.lock().following.remove(&self.key);
    }
}

/// 一个在等回应的请求：没等到就不等了的，拿掉它，要发取消的发。
struct Asking<'a> {
    link: &'a Link,
    id: u64,
    cancel: bool,
    done: bool,
}

impl Drop for Asking<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        let was_waiting = self.link.shared.lock().asked.remove(&self.id).is_some();
        if was_waiting && self.cancel {
            self.link.tell(
                "notifications/cancelled",
                json!({"requestId": self.id, "reason": "the caller stopped waiting"}),
            );
        }
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Waiting> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 断了：在等的都放下（它们收到 [`Failed::Closed`]），之后发的当场收到。
    fn close(&self) {
        let asked = {
            let mut waiting = self.lock();
            waiting.open = false;
            std::mem::take(&mut waiting.asked)
        };
        drop(asked);
        self.closed.send_replace(true);
    }
}

/// 写的一头：一行行写进服务的标准输入。写不进去就断了。连接丢下了（没有要写的了），关掉服务的标准输入：MCP 的 stdio 关法，
/// 服务读到头自己退出。
async fn write<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut lines: mpsc::Receiver<Vec<u8>>,
    shared: Arc<Shared>,
) {
    while let Some(line) = lines.recv().await {
        let written = async {
            writer.write_all(&line).await?;
            writer.flush().await
        };
        if let Err(error) = written.await {
            tracing::debug!(target: TARGET, error = %error, "MCP server input closed");
            shared.close();
            return;
        }
    }
    if let Err(error) = writer.shutdown().await {
        tracing::debug!(target: TARGET, error = %error, "MCP server input not shut");
    }
}

/// 读的一头：一行行认，回应交给在等的，服务的请求当场回，通知照种类办。读到头、太长、读不了就断了；连接丢下了的，服务再问
/// 什么也不回、断了。
async fn read<R: AsyncRead + Unpin>(
    reader: R,
    out: mpsc::WeakSender<Vec<u8>>,
    shared: Arc<Shared>,
) {
    let mut reader = BufReader::new(reader);
    loop {
        let line = match wire::read_line(&mut reader).await {
            Ok(Read::Line(line)) => line,
            Ok(Read::Closed) => break,
            Ok(Read::TooLong) => {
                tracing::warn!(target: TARGET, limit = wire::LINE_LIMIT, "MCP line too long, closing");
                break;
            }
            Err(error) => {
                tracing::debug!(target: TARGET, error = %error, "MCP server output unreadable");
                break;
            }
        };
        match wire::parse(&line) {
            Incoming::Answer(id, outcome) => {
                let reply = shared.lock().asked.remove(&id);
                if let Some(reply) = reply
                    && reply.send(outcome).is_err()
                {
                    tracing::debug!(target: TARGET, id, "answer arrived after the caller left");
                }
            }
            Incoming::Request(id, method) => {
                let outcome = match method.as_str() {
                    "ping" => Ok(json!({})),
                    _ => {
                        tracing::info!(target: TARGET, method = method.as_str(), "MCP server request refused");
                        Err((METHOD_NOT_FOUND, "Method not found"))
                    }
                };
                let Some(out) = out.upgrade() else {
                    break;
                };
                match out.try_send(wire::answer(&id, outcome)) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        tracing::debug!(target: TARGET, method = method.as_str(), "answer to the MCP server dropped: queue full");
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => break,
                }
            }
            Incoming::Notice(method, params) => match method.as_str() {
                // 工具、提示词（施工 X-39）变了数同一个：变了就都再列一次。新版服务经 `subscriptions/listen` 推的一样（施工 PT-1 下）。
                "notifications/tools/list_changed" | "notifications/prompts/list_changed" => {
                    shared.changes.send_modify(|count| *count += 1);
                }
                "notifications/progress" => {
                    let key = params.get("progressToken").map(Value::to_string);
                    let follower = key.and_then(|key| shared.lock().following.get(&key).cloned());
                    if let Some(follower) = follower
                        && let Err(error) = follower.try_send(params)
                    {
                        tracing::debug!(target: TARGET, error = %error, "progress dropped");
                    }
                }
                _ => tracing::trace!(target: TARGET, method = method.as_str(), "MCP notice"),
            },
            Incoming::Bad(problem) => {
                tracing::warn!(target: TARGET, problem = problem.as_str(), "MCP line not understood");
            }
        }
    }
    shared.close();
}

/// 服务回的错：错误码、原话、附带的；写法不对的照写法不对报。
fn rpc(error: Value) -> Failed {
    let code = error.get("code").and_then(Value::as_i64);
    let message = error.get("message").and_then(Value::as_str);
    match (code, message) {
        (Some(code), Some(message)) => Failed::Rpc {
            code,
            message: message.to_string(),
            data: error.get("data").cloned(),
        },
        _ => Failed::Malformed(format!("an error without code or message: {error}")),
    }
}

#[cfg(test)]
mod tests;
