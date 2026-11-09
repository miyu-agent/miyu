//! 推给宿主页的（施工 BP-3）：`GET /e/<票>` 是一条 `text/event-stream`，每条 `data:` 是一条要转进框的 JSON-RPC 推送，宿主页照原样交
//! 给框。照网页 `frame.js` 转：`config.changed` 里有它的项的推 `settings.changed {"keys"}`；它的程序推的 `package.event` 照原样；
//! `package` 流掉了队的重订、再推 `resync`。开着的推送流算服务没闲着；隔一会儿推一行注释，断了的早点知道。

use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body::{Body, Frame};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

use super::Running;
use super::link::Link;

/// 最久多久推一行注释：断了的推送流写不进去才认得出来（服务闲置期短的照它的一半，`Running::beat`）。
pub(super) const BEAT: Duration = Duration::from_secs(30);

/// 一条推送流的回应正文。
pub(super) struct Events {
    rx: mpsc::Receiver<Bytes>,
    _held: Held,
}

/// 开着的这条推送流：丢了算少一条。
struct Held(Arc<Running>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.streams.fetch_sub(1, Ordering::SeqCst);
        self.0.touch();
    }
}

impl Body for Events {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.rx
            .poll_recv(cx)
            .map(|chunk| chunk.map(|chunk| Ok(Frame::data(chunk))))
    }
}

/// 开一条：先订核心的配置、这个包的推送，再一条条转。
pub(super) async fn open(link: Arc<Link>, package: String, running: Arc<Running>) -> Events {
    running.streams.fetch_add(1, Ordering::SeqCst);
    let held = Held(running);
    let pushes = link.pushes();
    follow(&link, &package, true).await;
    let (tx, rx) = mpsc::channel(64);
    let beat = held.0.beat;
    tokio::spawn(forward(link, package, pushes, tx, beat));
    Events { rx, _held: held }
}

/// 订这个包的推送；`config` 也订的连配置一起。旧核心、没装的包订不上的不报错，框照旧能用。
async fn follow(link: &Link, package: &str, config: bool) {
    if config {
        drop(link.request("subscribe", json!({"stream": "config"})).await);
    }
    drop(
        link.request(
            "subscribe",
            json!({"stream": "package", "package": package}),
        )
        .await,
    );
}

async fn forward(
    link: Arc<Link>,
    package: String,
    mut pushes: broadcast::Receiver<Value>,
    tx: mpsc::Sender<Bytes>,
    every: Duration,
) {
    let mut beat = tokio::time::interval(every);
    beat.tick().await;
    loop {
        let out = tokio::select! {
            () = tx.closed() => return,
            _ = beat.tick() => Some(Bytes::from_static(b": beat\n\n")),
            got = pushes.recv() => match got {
                Ok(push) => translated(&link, &package, &push).await.map(|push| line(&push)),
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    follow(&link, &package, false).await;
                    Some(line(&resync(&package)))
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
        };
        if let Some(out) = out
            && tx.send(out).await.is_err()
        {
            return;
        }
    }
}

/// 核心的一条推送换成要转进框的；不相干的没有。
async fn translated(link: &Link, package: &str, push: &Value) -> Option<Value> {
    let params = &push["params"];
    match push["method"].as_str()? {
        "package.event" if params["package"] == package => Some(push.clone()),
        "resync" if params["stream"] == "package" && params["package"] == package => {
            follow(link, package, false).await;
            Some(resync(package))
        }
        "resync" if params["stream"] == "config" => {
            drop(link.request("subscribe", json!({"stream": "config"})).await);
            None
        }
        "config.changed" => settings_changed(package, params),
        _ => None,
    }
}

/// `config.changed` 里它的项（`<编号>.` 开头的键）：推 `settings.changed {"keys"}`；一项都没有的不推。
fn settings_changed(package: &str, params: &Value) -> Option<Value> {
    let prefix = format!("{package}.");
    let mine: Vec<&String> = params["keys"]
        .as_object()?
        .keys()
        .filter(|key| key.starts_with(&prefix))
        .collect();
    (!mine.is_empty())
        .then(|| json!({"jsonrpc": "2.0", "method": "settings.changed", "params": {"keys": mine}}))
}

fn resync(package: &str) -> Value {
    json!({"jsonrpc": "2.0", "method": "resync", "params": {"stream": "package", "package": package}})
}

fn line(push: &Value) -> Bytes {
    Bytes::from(format!("data: {push}\n\n"))
}

#[cfg(test)]
mod tests;
