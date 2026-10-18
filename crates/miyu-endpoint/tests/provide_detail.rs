//! 提供者的工具带给头看的结构化结果（施工 W-11 补，`providers.md`、`docs/reference/extensions.md` 的 `tool.call`）：合通用形状的
//! 原样记进结果的 `detail`；形状不对的、出错时带的丢掉，结果照常。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"]})
    };
    json!({"tools": [spec("links"), spec("bad_detail"), spec("detail_error")]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn an_extension_tool_detail_in_a_known_shape_reaches_the_log() {
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
        Play::calls(&[
            ("links", "{}"),
            ("bad_detail", "{}"),
            ("detail_error", "{}"),
        ]),
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

    client.say("s1", &session, "找一找").await;
    home.until_turns(&session, 1).await;

    let details: Vec<Option<Value>> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(
                result
                    .detail
                    .as_ref()
                    .map(|raw| serde_json::from_str(raw.get()).expect("是 JSON")),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        details,
        [
            Some(json!({"kind": "links", "links": [
                {"title": "Miyu", "url": "https://example.com/miyu", "snippet": "A cat."}
            ]})),
            None,
            None,
        ],
        "合形状的照形状重写了记（写成 null 的那一格去掉）；不是 http 的网址整份丢掉；出错的不记"
    );
}
