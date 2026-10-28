//! MCP 工具的进度上视图流（施工 PT-1 下）：真核心真起 `miyu-test-mcp steps`，调 `steps` 时带进度令牌，服务推的一条进度照脚本
//! 的进度行推给头，视图流里那一步带上 `1/2 half`；结果到了那一条不带进度。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 发一句话，读到它的回应、会话状态回到闲着：交回之间推来的。
async fn say_through(client: &mut Client, id: &str, session: &str, text: &str) -> Vec<Value> {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "session.send",
        "params": {"session": session, "text": text}});
    client.line(&request.to_string()).await;
    let mut pushed = Vec::new();
    let (mut replied, mut ended) = (false, false);
    while !(replied && ended) {
        let next = client.next().await.expect("没断开");
        if next["id"] == json!(id) {
            replied = true;
            continue;
        }
        ended |= next["method"] == json!("view.status")
            && next["params"]["status"]["state"] == json!("idle");
        pushed.push(next);
    }
    pushed
}

#[tokio::test]
async fn progress_from_an_mcp_tool_reaches_the_view_stream() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &["steps"]);
    let script = Script::new([
        Play::calls(&[("mcp__probe__steps", "{}")]),
        Play::Says("好。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = enable(&mut client, "probe").await;
    assert_eq!(enabled["result"]["enabled"], true, "{enabled}");
    until_status(&mut client, "probe", "running").await;
    let session = open(&mut client).await;
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    let pushed = say_through(&mut client, "c3", &session, "跑一下").await;
    let updates: Vec<(Option<String>, String)> = pushed
        .iter()
        .filter(|push| push["method"] == json!("view.update"))
        .map(|push| &push["params"]["entry"])
        .filter(|entry| entry["kind"] == json!("tool"))
        .map(|entry: &Value| {
            (
                entry["progress"].as_str().map(str::to_string),
                entry["state"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert!(
        updates
            .iter()
            .any(|(progress, _)| progress.as_deref() == Some("1/2 half")),
        "推过进度：{updates:?}"
    );
    let (last, state) = updates.last().expect("有更新");
    assert_eq!(state, "ok", "{updates:?}");
    assert_eq!(*last, None, "结果到了不带进度：{updates:?}");
}
