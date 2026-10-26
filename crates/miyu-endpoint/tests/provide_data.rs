//! 提供者的工具带给脚本的结构化结果（施工 X-33，`providers.md`、`docs/reference/extensions.md` 的 `tool.call`）：脚本里调到的
//! 拿到原样的 `data`；出错的不交；直接调用的 `tool.result` 里没有它。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"], "clicall": true})
    };
    json!({"tools": [spec("data"), spec("data_error")]})
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
async fn an_extension_tool_data_reaches_scripts_only() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    install_providing(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{}", tools()), "serve"]),
    );
    // 假的 `shell`：命令里经端口调这两件，答回来的一件一行。
    let shell = Fake::commanding(
        "shell",
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("data", "{}"), ("data_error", "{}")]),
    );
    let catalog = Catalog::new([Arc::clone(&shell) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([
        Play::calls(&[("data", "{}")]),
        Play::calls(&[("shell", r#"{"command":"python3 x.py"}"#)]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, catalog, None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;

    client.say("s1", &session, "找一找").await;
    home.until_turns(&session, 1).await;

    let results: Vec<Value> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(serde_json::to_value(result).expect("写得成 JSON")),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 2, "{results:?}");
    let direct = results[0].to_string();
    assert!(direct.contains("found data"), "{direct}");
    assert!(
        !direct.contains("items"),
        "直接调用的结果里没有 data：{direct}"
    );
    assert_eq!(
        results[1]["blocks"][0]["text"],
        "data ok text=found data data={\"empty\":[],\"items\":[{\"n\":1}]}\ndata_error error text=failed",
        "脚本拿到原样的 data，出错的不交"
    );
}
