//! 新时代的订阅（施工 PT-1 下，规范 2026-07-28「Subscriptions」）：服务声明了 `listChanged` 的，客户端在后台开一条
//! `subscriptions/listen`，推来的「变了」由 [`Link`] 照常数。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::error::Failed;
use crate::link::Link;

/// 「没有这个方法」：服务不认 `subscriptions/listen` 的不再开。
const METHOD_NOT_FOUND: i64 = -32601;

/// 订阅断了以后先等多久再开，每次翻倍，最多等多久；开着超过多久才断的从头等起（施工 PT-1 下）。
const LISTEN_PAUSE: Duration = Duration::from_secs(1);
const LISTEN_MOST: Duration = Duration::from_secs(60);
const LISTEN_STEADY: Duration = Duration::from_secs(60);

/// 服务声明了哪几种「变了」（施工 PT-1 下）：本事里工具、提示词的 `listChanged` 是真的那几种，写成 `subscriptions/listen` 的
/// `notifications`；一种都没有的没有。
pub(super) fn listens(result: &Value) -> Option<Value> {
    let capabilities = &result["capabilities"];
    let mut wanted = Map::new();
    if capabilities["tools"]["listChanged"] == true {
        wanted.insert("toolsListChanged".to_string(), json!(true));
    }
    if capabilities["prompts"]["listChanged"] == true {
        wanted.insert("promptsListChanged".to_string(), json!(true));
    }
    (!wanted.is_empty()).then_some(Value::Object(wanted))
}

/// 开着一条 `subscriptions/listen`（施工 PT-1 下）：推来的通知 [`Link`] 照常数；订阅结束了、出错了的退避着重开，服务不认这个
/// 方法的、连接断了的不再开。这个任务被停掉（丢下客户端）时，在等的请求照常发取消。
pub(super) async fn listen(link: Arc<Link>, params: Value) {
    let mut pause = LISTEN_PAUSE;
    loop {
        let began = tokio::time::Instant::now();
        match link
            .ask("subscriptions/listen", params.clone(), None, true)
            .await
        {
            Ok(_) => tracing::debug!(target: crate::TARGET, "MCP subscription ended, reopening"),
            Err(Failed::Closed) => return,
            Err(Failed::Rpc { code, .. }) if code == METHOD_NOT_FOUND => {
                tracing::info!(target: crate::TARGET, "MCP server does not take subscriptions/listen");
                return;
            }
            Err(error) => {
                tracing::info!(target: crate::TARGET, error = %error, "MCP subscription failed, reopening");
            }
        }
        if began.elapsed() > LISTEN_STEADY {
            pause = LISTEN_PAUSE;
        }
        tokio::time::sleep(pause).await;
        pause = (pause * 2).min(LISTEN_MOST);
    }
}
