//! 提供者的工具要结束这一轮（施工 4-14，`protocol.md` 的 `tool.call`）：回应带 `"end_turn": true`，核心记进结果、不再请求模型，
//! 这一轮照走完了结束；出错的照样带上的不认，接着请求。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, EndReason};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"]})
    };
    json!({"tools": [spec("skip"), spec("skip_error")]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 日志里结束了的几轮的原因，和结果上的 `end_turn`。
fn outcome(home: &Home, session: &str) -> (Vec<EndReason>, Vec<bool>) {
    let log = home.log(session);
    let ended = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnEnded(ended) => Some(ended.reason.clone()),
            _ => None,
        })
        .collect();
    let results = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.end_turn),
            _ => None,
        })
        .collect();
    (ended, results)
}

#[tokio::test]
async fn an_extension_tool_can_end_the_turn() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{}", tools()), "serve"]),
    );
    let script = Script::new([
        Play::calls(&[("skip", "{}")]),
        Play::calls(&[("skip_error", "{}")]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;

    client.say("s1", &session, "群里有人说话了").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        outcome(&home, &session),
        (vec![EndReason::Completed], vec![true]),
        "调了就结束"
    );
    assert_eq!(script.requests().len(), 1, "不再请求模型");

    client.say("s2", &session, "再来").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        outcome(&home, &session),
        (
            vec![EndReason::Completed, EndReason::Completed],
            vec![true, false]
        ),
        "出错的不认"
    );
    assert_eq!(script.requests().len(), 3, "出错的接着请求");
}
