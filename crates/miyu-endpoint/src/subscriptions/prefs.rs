//! 头自己的设置的订阅（施工 HP-1，`docs/blueprint/protocol.md`「头自己的设置」第 6 条）：一个连接一个包一个转发任务，再订阅换一个
//! 新的。订阅的回应（这一刻的整份）交给它先写，再转推送：别的包的、这个连接自己改的不推（第 5 条）。读得慢、掉了队，推一条
//! `resync`（`{"stream": "prefs", "package"}`），这个订阅就停了，头重新订阅。

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::Subscriptions;
use crate::Core;
use crate::prefs::Changed;
use crate::refusal::Refusal;

/// 一个包的设置的订阅的转发任务。
#[derive(Debug)]
pub(super) struct PrefsForwarder {
    /// 交订阅的回应给它的那一头：只交一次。
    reply: Option<oneshot::Sender<String>>,
    /// 转发任务：连接断了、换了新的、取消了跟着停。
    task: JoinHandle<()>,
}

impl Drop for PrefsForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Subscriptions {
    /// 订阅包 `package` 的设置：先查、再起新的转发任务替掉原来的，交回订阅的回应 `{"values": …}`，它经
    /// [`super::Target::Prefs`] 交给新的转发任务写出去。被拒的原来那一个照旧。
    ///
    /// # Errors
    ///
    /// 不是装着的界面包、文件读不了（`crate::prefs::follow`）。
    pub(crate) fn add_prefs(
        &mut self,
        core: &Core,
        package: &str,
        out: &mpsc::Sender<String>,
    ) -> Result<Value, Refusal> {
        let (snapshot, changes) = crate::prefs::follow(core, package)?;
        let (reply, answer) = oneshot::channel();
        let task = tokio::spawn(forward(package.to_string(), changes, answer, out.clone()));
        let forwarder = PrefsForwarder {
            reply: Some(reply),
            task,
        };
        drop(self.prefs.insert(package.to_string(), forwarder));
        Ok(snapshot)
    }

    /// 取消订阅包 `package` 的设置：转发任务当场停。没订阅着的不碍事。
    pub(crate) fn remove_prefs(&mut self, package: &str) {
        drop(self.prefs.remove(package));
    }

    /// 交包 `package` 的订阅的回应；没订阅着的、已经交过的，交回这一行。
    pub(super) fn reply_prefs(&mut self, package: &str, line: String) -> Result<(), String> {
        match self
            .prefs
            .get_mut(package)
            .and_then(|forwarder| forwarder.reply.take())
        {
            Some(reply) => reply.send(line),
            None => Err(line),
        }
    }
}

/// 转发：回应先写，再推这个包、不是这个连接改的；掉了队推 `resync` 停下。
async fn forward(
    package: String,
    mut changes: broadcast::Receiver<Arc<Changed>>,
    answer: oneshot::Receiver<String>,
    out: mpsc::Sender<String>,
) {
    let Ok(reply) = answer.await else {
        return;
    };
    if out.send(reply).await.is_err() {
        return;
    }
    loop {
        match changes.recv().await {
            Ok(changed) if changed.package != package || changed.from.writes_to(&out) => {}
            Ok(changed) => {
                if out.send(changed.line.clone()).await.is_err() {
                    break;
                }
            }
            Err(RecvError::Lagged(_)) => {
                tracing::warn!(target: "miyu::endpoint", stream = "prefs", "lagged, resync");
                let line = json!({"jsonrpc": "2.0", "method": "resync",
                    "params": {"stream": "prefs", "package": package}});
                if out.send(line.to_string()).await.is_err() {
                    // 连接断了：没人收。
                }
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}
