//! 假模型服务：本机回环上的 HTTP/1.1，照 openai-chat 的写法回（施工 V-1）。
//!
//! 每个 `POST` 都回同一段字（一块正文、一块收尾带用量、`[DONE]`），别的请求回一个空的模型列表。用量照请求体、正文的字节数
//! 估，四个字节一个 token；摘要请求（带出厂摘要指令的那一句）回一段摘要，好让长会话照真用时那样到线就压（施工 V-2 上）。它记下每个请求的头
//! 什么时候读全了：量尺照这个时刻算「说一句到模型收到请求」。驱动先把整份请求体编好才发头，所以头到的时刻已经包含了
//! 组装和编码；请求体在回环上传多久不算进去。
//!
//! 回答默认是一块正文；流式那一场（施工 V-22）照 [`Fake::split_into`] 切成几百块，像真模型那样一点点来。工具一步那一场
//! （施工 V-29）照 [`Fake::script`] 给的剧本回工具调用（`fake/script.rs`），每个回答写完（`[DONE]` 写出去）也记下时刻；
//! 给了 [`Fake::probe`] 的，头读全、回答写完的那一刻各读一次核心花的 CPU。
//!
//! 和 `miyu_http::testkit` 的假服务器不同：那个照剧本回、回完就不收了；这里一直收，回的字多长由量尺定。

pub mod script;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

use crate::cpu::{Cpu, Probe};
use crate::rpc::PATIENCE;
use script::Script;

/// 收到的一个请求：头读全的时刻，和请求体。
pub struct Arrival {
    /// 请求头读全的时刻。
    pub at: Instant,
    /// 同一刻的墙上时间：和核心的日志、会话日志里的时刻对（施工 V-29）。
    pub wall: SystemTime,
    /// 头读全以后读的核心 CPU（施工 V-29）：没给 [`Fake::probe`] 的是 `None`。
    pub cpu: Option<Cpu>,
    /// 照剧本回的（施工 V-29）：请求体里已经有前几次调用的结果；不归剧本的是 `None`。
    pub step: Option<usize>,
    /// 请求体。
    pub body: Vec<u8>,
    /// 回答写完时交来（施工 V-29）；没写完（连接断了）的不交。
    pub answered: oneshot::Receiver<Answered>,
}

/// 一个回答写完了（施工 V-29）：整段事件流连同 `[DONE]` 交给了系统的那一刻。
#[derive(Debug, Clone, Copy)]
pub struct Answered {
    /// 写完的时刻。
    pub at: Instant,
    /// 同一刻的墙上时间。
    pub wall: SystemTime,
    /// 写完以后读的核心 CPU：没给 [`Fake::probe`] 的是 `None`。
    pub cpu: Option<Cpu>,
}

/// 几个连接共用的设定（施工 V-29）：剧本、读谁的 CPU。
#[derive(Default)]
struct Setup {
    script: Option<Script>,
    probe: Option<Probe>,
}

/// 跑着的假服务。
pub struct Fake {
    /// 给核心写进配置的地址：`http://127.0.0.1:<端口>/v1`。
    pub base_url: String,
    arrivals: mpsc::UnboundedReceiver<Arrival>,
    /// 之后的回答正文切成几块（施工 V-22）。
    pieces: Arc<AtomicUsize>,
    /// 剧本、读谁的 CPU（施工 V-29）。
    setup: Arc<Mutex<Setup>>,
}

impl Fake {
    /// 在本机回环上随便挑个端口起来。每个回答的正文是 `reply_bytes` 个字节上下的字。
    ///
    /// # Errors
    ///
    /// 绑不上端口。
    pub async fn start(reply_bytes: usize) -> Result<Fake, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("假模型服务绑不上端口：{e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("假模型服务的端口读不出：{e}"))?
            .port();
        let reply: Arc<str> = filler(reply_bytes).into();
        let (sender, arrivals) = mpsc::unbounded_channel();
        let pieces = Arc::new(AtomicUsize::new(1));
        let split = Arc::clone(&pieces);
        let setup = Arc::new(Mutex::new(Setup::default()));
        let shared = Arc::clone(&setup);
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let pieces = split.load(Ordering::Relaxed);
                let reply = Arc::clone(&reply);
                tokio::spawn(serve(
                    socket,
                    reply,
                    pieces,
                    Arc::clone(&shared),
                    sender.clone(),
                ));
            }
        });
        Ok(Fake {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            arrivals,
            pieces,
            setup,
        })
    }

    /// 之后提到剧本记号的请求照剧本回工具调用（施工 V-29）；`None` 收回。
    pub fn script(&self, script: Option<Script>) {
        self.setup
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .script = script;
    }

    /// 之后每个请求头读全、每个回答写完时读一次 `probe` 的 CPU（施工 V-29）；`None` 不读。
    pub fn probe(&self, probe: Option<Probe>) {
        self.setup
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .probe = probe;
    }

    /// 之后的回答正文切成 `pieces` 块（至少一块），一块一个事件（施工 V-22）：量流式时核心每收一块做了多少。
    pub fn split_into(&self, pieces: usize) {
        self.pieces.store(pieces.max(1), Ordering::Relaxed);
    }

    /// 等第一个提到 `marker` 的请求，交回它；之前到的别的请求（起标题这些辅助请求）跳过。
    ///
    /// # Errors
    ///
    /// 服务停了、等太久。
    pub async fn request_with(&mut self, marker: &str) -> Result<Arrival, String> {
        let waiting = async {
            while let Some(arrival) = self.arrivals.recv().await {
                if mentions(&arrival.body, marker.as_bytes()) {
                    return Ok(arrival);
                }
            }
            Err("假模型服务停了".to_string())
        };
        tokio::time::timeout(PATIENCE, waiting).await.map_err(|_| {
            format!(
                "等了 {} 秒，模型没收到带「{marker}」的请求",
                PATIENCE.as_secs()
            )
        })?
    }
}

/// 一个连接：读一个请求，记下，回完关上，记下回完的时刻。正文切成 `pieces` 块；归剧本的照剧本回（施工 V-29）。
async fn serve(
    mut socket: TcpStream,
    reply: Arc<str>,
    pieces: usize,
    setup: Arc<Mutex<Setup>>,
    arrivals: mpsc::UnboundedSender<Arrival>,
) {
    let Some((head, body, at)) = read_request(&mut socket).await else {
        return;
    };
    let wall = SystemTime::now();
    // 剧本、读谁照这一刻的；读 CPU 放在记下时刻之后，读的那一点不算进核心的这一段。
    let (scripted, probe) = {
        let setup = setup.lock().unwrap_or_else(PoisonError::into_inner);
        let scripted = setup
            .script
            .as_ref()
            .and_then(|script| Some((script.clone(), script.answered(&body)?)));
        (scripted, setup.probe.clone())
    };
    let cpu = probe.as_ref().map(Probe::sample);
    let response = if head.method == "POST" {
        let mut response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                .to_vec();
        let stream = match &scripted {
            Some((script, done)) => script.answer(&body, *done),
            None => answer(&body, &reply, pieces),
        };
        response.extend_from_slice(stream.as_bytes());
        response
    } else {
        let list = r#"{"object":"list","data":[]}"#;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{list}",
            list.len()
        )
        .into_bytes()
    };
    // 先记下再回：量尺等到这一条时，核心那边正在收回答，不会因为记晚了少算。
    let (written, answered) = oneshot::channel();
    let step = scripted.map(|(_, done)| done);
    let arrival = Arrival {
        at,
        wall,
        cpu,
        step,
        body,
        answered,
    };
    if arrivals.send(arrival).is_err() {
        return;
    }
    if socket.write_all(&response).await.is_ok() {
        let at = Instant::now();
        let wall = SystemTime::now();
        let cpu = probe.as_ref().map(Probe::sample);
        #[expect(clippy::let_underscore_must_use, reason = "量尺不等这一次的就没人收")]
        let _ = written.send(Answered { at, wall, cpu });
        #[expect(clippy::let_underscore_must_use, reason = "回完了，关不干净也不影响量")]
        let _ = socket.shutdown().await;
    }
}

/// 读一个请求：头读全记下时刻，再照 `Content-Length` 读完请求体。连接断了、头读不懂的是 `None`。
async fn read_request(socket: &mut TcpStream) -> Option<(Head, Vec<u8>, Instant)> {
    let mut data = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let (head, at) = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..read]);
        if let Some(head) = parse_head(&data) {
            break (head, Instant::now());
        }
    };
    let mut body = data.split_off(head.size);
    while body.len() < head.length {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Some((head, body, at))
}

/// 一个请求的头：方法、请求体多长、头本身占几个字节（连同空行）。
#[derive(Debug, PartialEq, Eq)]
pub struct Head {
    /// `GET`、`POST`……
    pub method: String,
    /// `Content-Length`，没写的是 0。
    pub length: usize,
    /// 头占几个字节，连同结尾的空行：请求体从这里开始。
    pub size: usize,
}

/// 读出请求头；还没读到空行的是 `None`。
pub fn parse_head(data: &[u8]) -> Option<Head> {
    let end = data.windows(4).position(|window| window == b"\r\n\r\n")?;
    let text = String::from_utf8_lossy(&data[..end]);
    let mut lines = text.split("\r\n");
    let method = lines.next()?.split(' ').next()?.to_string();
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    Some(Head {
        method,
        length,
        size: end + 4,
    })
}

/// 请求体里有没有 `marker`。从后往前找：人说的最后一句排在请求后面，长会话的请求体有几兆，从前找要白扫一遍。
pub fn mentions(body: &[u8], marker: &[u8]) -> bool {
    !marker.is_empty()
        && body
            .windows(marker.len())
            .rev()
            .any(|window| window == marker)
}

/// 摘要请求认的那一句：出厂摘要指令（`resources/core/compaction/summarize-task.txt`）第二段的开头。
const SUMMARY_MARKER: &[u8] = b"Write a detailed summary of the conversation above";

/// 量尺的摘要：草稿一句、摘要一段，几百字节。
const SUMMARY: &str = "<analysis>量尺的摘要草稿。</analysis>\n<summary>量尺的摘要：用户一直在让模型回长段的字，好让会话长大；没有要接着做的事，也没有改过的文件。The measuring session only grows; nothing is pending.</summary>";

/// 照请求体 `body` 回的事件流（施工 V-2 上）：摘要请求回 [`SUMMARY`]（一块），别的回 `text`，切成 `pieces` 块；用量照
/// 请求体、正文的字节数估。
pub fn answer(body: &[u8], text: &str, pieces: usize) -> String {
    let (said, pieces) = match mentions(body, SUMMARY_MARKER) {
        true => (SUMMARY, 1),
        false => (text, pieces),
    };
    stream_pieces(said, pieces, tokens(body.len()), tokens(said.len()))
}

/// 字节数折成 token：四个字节一个，往上取整。
fn tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

/// 回答的事件流：正文照字切成 `pieces` 块（至少一块、不切开一个字；字比块少的一个字一块），一块一个事件，头一块带
/// `role`；收尾一块带用量（输入 `prompt`、输出 `completion` 个 token），最后 `[DONE]`。
pub fn stream_pieces(text: &str, pieces: usize, prompt: usize, completion: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let size = chars.len().div_ceil(pieces.max(1)).max(1);
    let mut stream = String::new();
    for (n, piece) in chars.chunks(size).enumerate() {
        let piece: String = piece.iter().collect();
        let delta = match n {
            0 => json!({"role": "assistant", "content": piece}),
            _ => json!({"content": piece}),
        };
        let said = json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]});
        stream.push_str(&format!("data: {said}\n\n"));
    }
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": prompt, "completion_tokens": completion,
            "total_tokens": prompt + completion}});
    stream.push_str(&format!("data: {done}\n\ndata: [DONE]\n\n"));
    stream
}

/// 回答的正文：一段中英混排的字反复，凑到 `bytes` 个字节上下（不切开一个字）。
pub fn filler(bytes: usize) -> String {
    const PIECE: &str = "量尺的回答：这一段字只是占地方，让会话照真用时的样子长大。The measuring reply only takes up room so the session grows the way a real one does. ";
    let mut text = String::with_capacity(bytes + PIECE.len());
    while text.len() < bytes {
        text.push_str(PIECE);
    }
    text
}

#[cfg(test)]
mod tests;
