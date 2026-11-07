//! 反向调用（施工 O-2 上，`docs/blueprint/providers.md`「怎么走」第 1、2 条）：核心发给一个连接的请求，编号 `core-<n>`，照连接
//! 从 1 数起；对上编号的回应交给等它的，对不上的不理。连接断了（[`Peer::close`]），在等的和以后发的都了结成 [`Gone`]。不等了的
//! （等它的 future 被丢掉）从表里拿掉；通知（[`Peer::notify`]，`tool.cancel`）不带编号、不等回应（施工 O-2 下）。
//!
//! 扩展提供的模型驱动（施工 MD-2，设计 66 第五节）：`driver.call` 的参数原样拼进那一行（[`Peer::call_raw`]，1 MB 的请求不多解析
//! 一遍）；扩展发来的 `driver.delta` 照 `call` 交给登记了这一次的那一头（[`Peer::stream`]、[`Peer::delta`]），对不上的丢掉。
//! 读的那一头先交增量、再交回应，等回应的拿到回应时增量已经都交了。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::wire::Response;

/// 连接断了：发不出去，或者等不到回应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gone;

/// 一个连接上核心这边的一头：往它发请求、收它的回应。克隆的是同一个。
#[derive(Clone)]
pub(crate) struct Peer {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Peer")
    }
}

/// 发出去还没回的。
#[derive(Default)]
struct State {
    /// 上一条的编号。
    last: u64,
    /// 编号到等它的。
    waiting: HashMap<String, oneshot::Sender<Result<Value, Value>>>,
    /// 扩展驱动的调用编号到交增量的那一头（施工 MD-2）：连接断了一起放掉。
    streams: HashMap<String, mpsc::UnboundedSender<Value>>,
    /// 往连接写的一头；连接断了放掉，是 `None`（施工 O-2 再补：提供者表里存着这一头，留着它，写的那一头就一直不结束，
    /// 核心察觉不到扩展退出，崩了不重新拉起）。
    out: Option<mpsc::Sender<String>>,
}

impl Peer {
    /// 往 `out` 写的一头。
    pub(crate) fn new(out: mpsc::Sender<String>) -> Peer {
        Peer {
            state: Arc::new(Mutex::new(State {
                out: Some(out),
                ..State::default()
            })),
        }
    }

    /// 发一条请求，等它的回应：`result`，或者 `error` 的原样。
    ///
    /// # Errors
    ///
    /// 连接断了。
    pub(crate) async fn call(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, Value>, Gone> {
        self.call_raw(method, params.to_string()).await
    }

    /// 同 [`Peer::call`]，参数是写好的原文（一个 JSON 对象，施工 MD-2）：原样拼进那一行，不再解析。
    ///
    /// # Errors
    ///
    /// 连接断了。
    pub(crate) async fn call_raw(
        &self,
        method: &str,
        params: String,
    ) -> Result<Result<Value, Value>, Gone> {
        let (id, answer, out) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(out) = state.out.clone() else {
                return Err(Gone);
            };
            state.last += 1;
            let id = format!("core-{}", state.last);
            let (tell, answer) = oneshot::channel();
            state.waiting.insert(id.clone(), tell);
            (id, answer, out)
        };
        // 不等了的（这个 future 被丢掉、发不出去、等到了）都从表里拿掉。
        let _waiting = Waiting {
            peer: self,
            id: &id,
        };
        let head = json!({"jsonrpc": "2.0", "id": id, "method": method}).to_string();
        // `head` 是一个不空的对象：去掉最后的 `}`，接上 `"params":<原文>}`。
        let line = format!("{},\"params\":{params}}}", &head[..head.len() - 1]);
        if out.send(line).await.is_err() {
            return Err(Gone);
        }
        answer.await.map_err(|_| Gone)
    }

    /// 发一条通知（施工 O-2 下）：不带编号、不等回应。连接断了、写队列满了的发不出去，交回 `false`。
    pub(crate) fn notify(&self, method: &str, params: Value) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(out) = state.out.as_ref() else {
            return false;
        };
        let line = json!({"jsonrpc": "2.0", "method": method, "params": params});
        out.try_send(line.to_string()).is_ok()
    }

    /// 发一条通知，写队列满了等空位（施工 HK-2：`tool.after` 的发送任务，不在会话的路上）。连接断了的交回 `false`。
    pub(crate) async fn notify_waiting(&self, method: &str, params: Value) -> bool {
        let out = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(out) = state.out.clone() else {
                return false;
            };
            out
        };
        let line = json!({"jsonrpc": "2.0", "method": method, "params": params});
        out.send(line.to_string()).await.is_ok()
    }

    /// 收到一条回应：交给等它的，对不上的不理。
    pub(crate) fn answer(&self, response: Response) {
        let tell = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting
            .remove(&response.id);
        if let Some(tell) = tell {
            drop(tell.send(response.outcome));
        }
    }

    /// 连接断了：在等的都了结，以后发的直接了结；往连接写的那一头放掉，写的任务才结束得了（施工 O-2 再补）。交增量的那几头一起放掉
    /// （施工 MD-2）。
    pub(crate) fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.waiting.clear();
        state.streams.clear();
        state.out = None;
    }

    /// 扩展驱动的调用 `call` 的增量交给 `deltas`（施工 MD-2）：登记到不要了为止（[`Peer::unstream`]）。
    pub(crate) fn stream(&self, call: &str, deltas: mpsc::UnboundedSender<Value>) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .streams
            .insert(call.to_string(), deltas);
    }

    /// 这一刻有几次扩展驱动的调用没答完（施工 MD-3）：`driver.check` 照它定能不能重启扩展。
    pub(crate) fn streaming(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .streams
            .len()
    }

    /// 调用 `call` 不要增量了：说完了、不等了。
    pub(crate) fn unstream(&self, call: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .streams
            .remove(call);
    }

    /// 收到一条 `driver.delta`（施工 MD-2）：照 `call` 把 `deltas` 原样交给登记了这一次的那一头；不认识的 `call`（说完了的、不等了的、
    /// 别人的）、写法不对的丢掉，不报错（设计 66 第五节第 2 条）。
    pub(crate) fn delta(&self, mut params: Value) {
        let Some(call) = params
            .get("call")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return;
        };
        let deltas = params
            .get_mut("deltas")
            .map(Value::take)
            .unwrap_or(Value::Null);
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(stream) = state.streams.get(&call) {
            drop(stream.send(deltas));
        }
    }

    /// 和 `other` 是同一个连接的一头（施工 IM-5：端口表照它认是谁登记的）。
    pub(crate) fn same(&self, other: &Peer) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    /// 往 `out` 这个写队列写（施工 HP-1：`prefs.changed` 照它认出改的那个连接，不推给它）。连接断了的（[`Peer::close`] 过）
    /// 谁都不是。
    pub(crate) fn writes_to(&self, out: &mpsc::Sender<String>) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .out
            .as_ref()
            .is_some_and(|mine| mine.same_channel(out))
    }

    /// 不再往它发了：连接断了（[`Peer::close`] 过，或者连接整个被丢下、写的那一头没了：核心停扩展时就是这样，施工 IM-5）。
    /// 找连接的都跳过它。
    pub(crate) fn closed(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.out.as_ref().is_none_or(mpsc::Sender::is_closed)
    }

    /// 不等这一条了。
    fn forget(&self, id: &str) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .waiting
            .remove(id);
    }
}

/// 在等的一条：丢掉时从表里拿掉。
struct Waiting<'a> {
    peer: &'a Peer,
    id: &'a str,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.peer.forget(self.id);
    }
}

#[cfg(test)]
mod tests;
