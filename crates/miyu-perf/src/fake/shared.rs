//! 三种传输共用的那一份（施工 LW-1）：收下读全的请求、定回应、交给量尺；传输只管读、写。原来写在明文的那个连接里，HTTPS
//! 的两个口开了以后收拢到这里，回的、记的三个口一样。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::{mpsc, oneshot};

use super::script::Script;
use super::{Answered, Arrival, Transport, answer};
use crate::cpu::Probe;

/// 几个连接共用的设定（施工 V-29）：剧本、读谁的 CPU；回答的事件之间隔多久（施工 MD-4）；整理记忆的请求照不照真的
/// 模型那样回、哪一个请求回 500（施工 LW-14）。
#[derive(Default)]
pub(super) struct Setup {
    pub(super) script: Option<Script>,
    pub(super) probe: Option<Probe>,
    pub(super) pace: Option<Duration>,
    pub(super) organize: bool,
    /// 下一个提到它、带工具表的请求回 500，回过一次就清掉（施工 LW-14）。
    pub(super) fail: Option<String>,
}

/// 几个口、所有连接共用的（施工 LW-1 从各个连接各拿一份收拢成一份）：回的正文、切几块、设定、交请求给量尺的那一头。
pub(super) struct Shared {
    pub(super) reply: String,
    /// 之后的回答正文切成几块（施工 V-22）。
    pub(super) pieces: AtomicUsize,
    pub(super) setup: Mutex<Setup>,
    pub(super) arrivals: mpsc::UnboundedSender<Arrival>,
}

/// 回 500 时的正文：照 openai-chat 的错误写法。
const FAILURE: &str =
    r#"{"error":{"message":"perf: the fake model failed once","type":"server_error"}}"#;

/// 传输读全的一个请求：交给 [`Shared::arrive`]。
pub(super) struct Request {
    /// 是不是 `POST`：是的回事件流，别的回空的模型列表。
    pub(super) post: bool,
    pub(super) body: Vec<u8>,
    /// 头读全的时刻。
    pub(super) at: Instant,
    pub(super) via: Transport,
}

/// 定好的回应：传输照它写，写完调 [`Reply::written`]。
pub(super) struct Reply {
    /// 回的是事件流（`POST`）还是 JSON 的模型列表。
    pub(super) stream: bool,
    /// 回的是 500（`Fake::fail_once` 要的，那个方法只在 Unix 上编，所以这里不写成链接）：正文是一段 JSON 的错误。
    pub(super) failed: bool,
    pub(super) body: String,
    pub(super) pace: Option<Duration>,
    pub(super) probe: Option<Probe>,
    pub(super) written: oneshot::Sender<Answered>,
}

impl Shared {
    /// 收下一个请求：照这一刻的剧本、切块、读谁的 CPU 定回应，先交给量尺再交回回应去写（量尺等到这一条时核心那边正在收
    /// 回答，不会因为记晚了少算）。量尺不收了的是 `None`。三种传输都走这里，回的、记的一样。
    pub(super) fn arrive(&self, request: Request) -> Option<Reply> {
        let wall = SystemTime::now();
        // 剧本、读谁照这一刻的；读 CPU 放在记下时刻之后，读的那一点不算进核心的这一段。
        let (scripted, probe, pace, organize, failed) = {
            let mut setup = self.setup.lock().unwrap_or_else(PoisonError::into_inner);
            let failed = request.post
                && setup.fail.as_ref().is_some_and(|marker| {
                    super::mentions(&request.body, marker.as_bytes())
                        && super::mentions(&request.body, b"\"tools\"")
                });
            if failed {
                setup.fail = None;
            }
            let scripted = setup
                .script
                .as_ref()
                .and_then(|script| Some((script.clone(), script.answered(&request.body)?)));
            (
                scripted,
                setup.probe.clone(),
                setup.pace,
                setup.organize,
                failed,
            )
        };
        let cpu = probe.as_ref().map(Probe::sample);
        let organized = organize
            .then(|| super::organizer::answer(&request.body))
            .flatten();
        let body = if failed {
            FAILURE.to_string()
        } else if request.post {
            match (&scripted, organized) {
                (Some((script, done)), _) => script.answer(&request.body, *done),
                (None, Some(text)) => super::stream_pieces(
                    &text,
                    1,
                    super::tokens(request.body.len()),
                    super::tokens(text.len()),
                ),
                (None, None) => answer(
                    &request.body,
                    &self.reply,
                    self.pieces.load(Ordering::Relaxed),
                ),
            }
        } else {
            r#"{"object":"list","data":[]}"#.to_string()
        };
        let (written, answered) = oneshot::channel();
        let arrival = Arrival {
            at: request.at,
            wall,
            cpu,
            step: scripted.map(|(_, done)| done),
            via: request.via,
            body: request.body,
            answered,
        };
        self.arrivals.send(arrival).ok()?;
        Some(Reply {
            stream: request.post && !failed,
            failed,
            body,
            pace,
            probe,
            written,
        })
    }
}

impl Reply {
    /// 状态行里的码和说法。
    pub(super) fn status(&self) -> (u16, &'static str) {
        match self.failed {
            true => (500, "Internal Server Error"),
            false => (200, "OK"),
        }
    }

    /// `Content-Type`。
    pub(super) fn content_type(&self) -> &'static str {
        match self.stream {
            true => "text/event-stream",
            false => "application/json",
        }
    }

    /// 照先后要写的几段：匀着的（施工 MD-4）一个事件（`\n\n` 收尾）一段，段之间隔 [`Reply::pace`]；不匀的一整段。
    pub(super) fn pieces(&self) -> Vec<&str> {
        match self.pace {
            Some(_) => self.body.split_inclusive("\n\n").collect(),
            None => vec![self.body.as_str()],
        }
    }

    /// 写完了：记下时刻、读一次 CPU，交给等着的量尺。
    pub(super) fn written(self) {
        let at = Instant::now();
        let wall = SystemTime::now();
        let cpu = self.probe.as_ref().map(Probe::sample);
        #[expect(clippy::let_underscore_must_use, reason = "量尺不等这一次的就没人收")]
        let _ = self.written.send(Answered { at, wall, cpu });
    }
}
