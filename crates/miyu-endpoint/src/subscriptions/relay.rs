//! 一个会话订阅的转发主循环（施工 BUS-2 从 `subscriptions.rs` 原样挪出来：那边到了 500 行）：回应和推送照先后放，掉队、会话
//! 停了推 `resync`。

use std::sync::Arc;

use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;
use miyu_session::{Ended, Pushed, Subscription};

use super::lines::{lines, resync};
use crate::Core;

/// 转发的主循环：交回来的时候，这个订阅不再推了（连接不要它了的，也交回来）。
pub(super) async fn relay(
    core: &Core,
    session: &SessionId,
    subscription: &mut Subscription,
    replies: &mut mpsc::UnboundedReceiver<String>,
    out: &mpsc::Sender<String>,
) {
    loop {
        tokio::select! {
            biased;
            reply = replies.recv() => {
                let Some(reply) = reply else {
                    return;
                };
                // 先放已经到了的推送；放着放着掉了队，这条回应也照样放出去再停。
                let mut alive = true;
                while let Some(next) = subscription.try_next() {
                    if !deliver(core, session, next, out).await {
                        alive = false;
                        break;
                    }
                }
                if out.send(reply).await.is_err() || !alive {
                    return;
                }
            }
            next = subscription.next() => {
                if !deliver(core, session, next, out).await {
                    return;
                }
            }
        }
    }
}

/// 交一份推送出去。这个订阅不再往下了（掉了队、会话停了、连接断了），交回 `false`；掉队的、会话停了的先推一条
/// `resync`。
async fn deliver(
    core: &Core,
    session: &SessionId,
    next: Result<Arc<Pushed>, Ended>,
    out: &mpsc::Sender<String>,
) -> bool {
    match next {
        Ok(pushed) => {
            for line in lines(core, session, &pushed) {
                if out.send(line).await.is_err() {
                    return false;
                }
            }
            true
        }
        Err(Ended::Lagged) => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), "lagged, resync");
            if out.send(resync(session)).await.is_err() {
                return false;
            }
            false
        }
        // 会话停了（施工 4-9 再补三上）：也推一条 `resync`，头知道这个订阅没了；重新订阅时，会话照会话表重新载入。
        Err(Ended::Stopped) => {
            tracing::info!(target: "miyu::endpoint", session = session.as_str(), "session stopped, resync");
            if out.send(resync(session)).await.is_err() {
                return false;
            }
            false
        }
    }
}
