//! 时间线上工具的名字（施工 9-8 六补）：真核心、真 MCP 服务走一遍。服务给了 `title` 的工具，调的那一刻记进结果（不分语言，记在
//! `en` 下，中文的连接没有中文的照它），视图的标题照它；服务没给的照工具名。服务关掉、工具出了目录，再翻照旧是记下的名字。
//! 扩展登记时写了 `display_name` 的同样记下，照连接的语言挑。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::mcp::*;
use crate::support::*;

/// 视图里一步步工具的标题，照先后。
async fn titles(client: &mut Client, id: &str, session: &str) -> Vec<String> {
    let reply = client
        .call(id, "view.page", json!({"session": session, "view": true}))
        .await;
    reply["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|entry| entry["kind"] == "tool")
        .map(|entry| entry["title"]["name"].as_str().unwrap_or("").to_string())
        .collect()
}

#[tokio::test]
async fn an_mcp_tool_step_is_titled_with_the_name_the_server_gave() {
    let home = Home::new();
    bundle(&home, "probe", "probe", &[]);
    let script = Script::new([
        Play::calls(&[
            ("mcp__probe__echo", r#"{"text":"hi"}"#),
            ("mcp__probe__fail", "{}"),
        ]),
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
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    let recorded: Vec<serde_json::Value> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(json!(result.display_name)),
            _ => None,
        })
        .collect();
    assert!(
        recorded.contains(&json!({"en": "Echo back"})) && recorded.contains(&json!({})),
        "给了的记下、没给的不记：{recorded:?}"
    );
    assert_eq!(
        titles(&mut client, "p1", &session).await,
        ["Echo back", "mcp__probe__fail"]
    );
    // 服务关掉、工具出了目录：照记下的。
    let off = client
        .call("d", "package.disable", json!({"package": "probe"}))
        .await;
    assert_eq!(off["result"]["enabled"], false, "{off}");
    until_status(&mut client, "probe", "off").await;
    assert_eq!(
        titles(&mut client, "p2", &session).await,
        ["Echo back", "mcp__probe__fail"]
    );
}

/// 扩展登记时写了 `display_name` 的工具（施工 9-8 六补，`providers.md`）：调的那一刻记进结果，视图的标题照连接的语言挑；没写的
/// 照工具名。
#[tokio::test]
async fn an_extension_tool_step_is_titled_with_the_name_it_registered() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"]})
    };
    let mut greet = spec("greet");
    greet["display_name"] = json!({"zh": "打招呼", "en": "Greet"});
    let tools = json!({"tools": [greet, spec("plain")]});
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{tools}"), "serve"]),
    );
    let script = Script::new([
        Play::calls(&[("greet", "{}"), ("plain", "{}")]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    until("扩展登记完", || read(&path).lines().count() >= 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "打个招呼").await;
    home.until_turns(&session, 1).await;
    let recorded: Vec<serde_json::Value> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(json!(result.display_name)),
            _ => None,
        })
        .collect();
    assert!(
        recorded.contains(&json!({"en": "Greet", "zh": "打招呼"})) && recorded.contains(&json!({})),
        "写了的记下、没写的不记：{recorded:?}"
    );
    // 测试的连接是中文的：挑中文的那一个。
    assert_eq!(
        titles(&mut client, "p1", &session).await,
        ["打招呼", "plain"]
    );
}
