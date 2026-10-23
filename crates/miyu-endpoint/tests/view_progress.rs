//! 工具进度上视图流（施工 X-45 补，`docs/blueprint/view.md` 的 `tool` 条目）：真的核心、真的 `shell`。命令先印一行进度，
//! 隔一会儿接着印两行，再睡 1.5 秒才结束：视图流里那一步带上进度，隔得近的合成一条、最后一条照样在结果之前到（压着的
//! 由转发循环到点推出来）；结果到了那一条不带进度。

use std::path::Path;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 这台机器上的命令：Unix 上是 bash、zsh 的，Windows 上是 PowerShell 的。
fn command() -> &'static str {
    if cfg!(windows) {
        "Write-Output 'miyu-progress: 1/3'; Start-Sleep -Milliseconds 100; Write-Output 'miyu-progress: 2/3'; Write-Output 'miyu-progress: 3/3'; Start-Sleep -Milliseconds 1500; Write-Output done"
    } else {
        "printf 'miyu-progress: 1/3\\n'; sleep 0.1; printf 'miyu-progress: 2/3\\nmiyu-progress: 3/3\\n'; sleep 1.5; echo done"
    }
}

/// 发一句话，读到它的回应、会话状态回到闲着：交回之间推来的（视图流的订阅换掉了事件流，等不到 `turn.ended`）。
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
async fn progress_reaches_the_view_stream_and_leaves_with_the_result() {
    let home = Home::new();
    let args = json!({"command": command(), "description": "Fetch"}).to_string();
    let script = Script::new([
        Play::calls(&[("shell", args.as_str())]),
        Play::Says("好了。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    // 这里测视图流，不测沙盒：完全放开，命令不用问人。
    client
        .call(
            "c2",
            "session.set_permission_level",
            json!({"session": session, "level": "full"}),
        )
        .await;
    client
        .call(
            "v1",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    let pushed = say_through(&mut client, "c3", &session, "抓一下").await;
    // 那一步（`tool`）的每一次更新：带的进度、状态。
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
    let progress: Vec<&str> = updates
        .iter()
        .filter_map(|(progress, _)| progress.as_deref())
        .collect();
    assert!(!progress.is_empty(), "推过进度：{updates:?}");
    assert!(progress.len() <= 2, "隔得近的合成一条：{updates:?}");
    assert_eq!(progress.last(), Some(&"3/3"), "最后一条照样到：{updates:?}");
    let (last, state) = updates.last().expect("有更新");
    assert_eq!(state, "ok", "{updates:?}");
    assert_eq!(*last, None, "结果到了不带进度：{updates:?}");
}
