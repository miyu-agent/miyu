//! 宿主页调核心的那条连接（施工 BP-3）：核心进程里的内存管道，另一头交给 [`crate::connection::serve`]，出示本机令牌握手，和
//! 终端连上来的同一套权限。一问一答照编号对上；推送（不带编号的）广播给开着的推送流。连接这一头丢了，管道关了，核心那一头
//! 跟着收掉。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, WriteHalf};
use tokio::sync::{broadcast, oneshot};

use crate::Core;
use crate::refusal::Refusal;

/// 管道一头多大：一条回应写不下也照样分几次读完，这里只管快慢。
const PIPE: usize = 256 * 1024;

/// 推送积着最多几条：推送流读得慢、掉了队的照 `resync` 办（`events.rs`）。
const PUSHES: usize = 256;

/// 等着回应的：编号到交回去的那一头。
type Pending = Mutex<HashMap<String, oneshot::Sender<Value>>>;

/// 一条连上、握过手的连接。
pub(super) struct Link {
    write: tokio::sync::Mutex<WriteHalf<DuplexStream>>,
    pending: Arc<Pending>,
    next: AtomicU64,
    pushes: broadcast::Sender<Value>,
}

impl Link {
    /// 连上 `core`、出示本机令牌握手。
    pub(super) async fn open(core: Arc<Core>) -> Result<Arc<Link>, Refusal> {
        let (ours, theirs) = tokio::io::duplex(PIPE);
        let token = core.token.clone();
        tokio::spawn(crate::connection::serve(theirs, core));
        let (read, write) = tokio::io::split(ours);
        let pending: Arc<Pending> = Arc::default();
        let (pushes, _) = broadcast::channel(PUSHES);
        tokio::spawn(read_lines(
            BufReader::new(read),
            Arc::clone(&pending),
            pushes.clone(),
        ));
        let link = Arc::new(Link {
            write: tokio::sync::Mutex::new(write),
            pending,
            next: AtomicU64::new(1),
            pushes,
        });
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": "page-host", "version": env!("CARGO_PKG_VERSION")},
            "caps": {"input": false},
            "token": token,
        });
        link.request("hello", hello)
            .await
            .map_err(|_| Refusal::INTERNAL)?;
        Ok(link)
    }

    /// 发一条请求、等回应：成了交回 `result`，拒了交回 `error` 那一格（原样）。管道断了的是内部错。
    pub(super) async fn request(&self, method: &str, params: Value) -> Result<Value, Value> {
        let id = format!("page-host-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone(), tx);
        let mut line =
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        line.push('\n');
        let written = self.write.lock().await.write_all(line.as_bytes()).await;
        let reply = match written {
            Ok(()) => rx.await.ok(),
            Err(_) => None,
        };
        let Some(mut reply) = reply else {
            self.pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id);
            return Err(json!({"code": -32603, "message": "core connection closed"}));
        };
        match reply.get_mut("error").map(Value::take) {
            Some(error) => Err(error),
            None => Ok(reply
                .get_mut("result")
                .map(Value::take)
                .unwrap_or(Value::Null)),
        }
    }

    /// 以后的推送。
    pub(super) fn pushes(&self) -> broadcast::Receiver<Value> {
        self.pushes.subscribe()
    }
}

/// 一行行读：带编号的交给等着的那一头，带方法、不带编号的广播出去。读到头（核心那一头收了）就停。
async fn read_lines(
    mut read: BufReader<tokio::io::ReadHalf<DuplexStream>>,
    pending: Arc<Pending>,
    pushes: broadcast::Sender<Value>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match read.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message.get("id").and_then(Value::as_str) {
            Some(id) => {
                let waiting = pending
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(id);
                if let Some(waiting) = waiting {
                    drop(waiting.send(message));
                }
            }
            None if message.get("method").is_some() => {
                drop(pushes.send(message));
            }
            None => {}
        }
    }
    pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}
