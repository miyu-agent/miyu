//! `venue.plugins`（施工 O-60，`venues.md`「场所插件」）：桥 `provide` 的工具写了 `plugin` 的，场所会话里桥没说开的不给，
//! 说开了下一轮给；同一个包里没写的照旧。`plugin` 写法不对的 `bad_tool`（`problem` 是 `plugin`）；别的平台的场所
//! `bad_params`（`field` 是 `venue`），名字写法不对的 `bad_params`（`field` 是 `on`）；本机的头（不是系统账号）回
//! `no_system_account`。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::id::AccountId;
use miyu_kernel::request::Request;
use miyu_session::testkit::{Play, Script};

use crate::support::extensions::*;
use crate::support::served::{replies, served_core};
use crate::support::*;

fn bot() -> AccountId {
    AccountId::parse("bot").expect("合写法")
}

fn names(request: &Request) -> Vec<String> {
    request.tools.iter().map(|tool| tool.name.clone()).collect()
}

/// 一件群里用的工具，`plugin` 照写（`null` 的不写）。
fn tool(name: &str, plugin: serde_json::Value) -> serde_json::Value {
    let mut tool = json!({"name": name, "description": "A test tool.",
        "input_schema": {"type": "object"}, "access": "venue", "venues": ["group"]});
    if !plugin.is_null() {
        tool["plugin"] = plugin;
    }
    tool
}

#[tokio::test]
async fn a_bridge_turns_on_its_plugin_tools_per_venue() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let first_done = home.work.join("first-done");
    let bad = json!({"tools": [tool("qq_bad", json!("Bad Name"))]});
    let good = json!({"tools": [tool("qq_mute", json!("qq-admin")), tool("qq_send", json!(null))]});
    let send =
        r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"qq:10001"}}"#;
    let args = steps(&[
        &step,
        "hello",
        &format!("ask:provide:{bad}"),
        &format!("ask:provide:{good}"),
        r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
        r#"ask:venue.plugins:{"venue":"tg:group:1","on":[]}"#,
        r#"ask:venue.plugins:{"venue":"qq:group:1","on":["Bad Name"]}"#,
        send,
        &format!("pause:{}", first_done.display()),
        r#"ask:venue.plugins:{"venue":"qq:group:1","on":["qq-admin"]}"#,
        send,
        "wait",
    ]);
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    home.write(
        "system/packages/bot/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n\n[command]\nname = \"bot\"\nprogram = \"{}\"\nabout = {{ en = \"B\" }}\n\n[process]\nargs = [{}]\nstart = \"always\"\nsystem_account = true\ncapabilities = {BRIDGE:?}\n\n[connection]\nplatform = \"qq\"\n",
            program.name(),
            args.join(", ")
        ),
    );
    approve(&home, "bot", &BRIDGE);
    let script = Script::new([Play::Says("在。"), Play::Says("在。")]);
    let core = served_core(&home, &script);

    let got = replies(&path, 7).await;
    assert_eq!(got[1]["error"]["data"]["reason"], "bad_tool", "{got:?}");
    assert_eq!(got[1]["error"]["data"]["problem"], "plugin", "{got:?}");
    assert_eq!(got[2]["result"], json!({"tools": 2}), "{got:?}");
    let session = got[3]["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    assert_eq!(got[4]["error"]["data"]["reason"], "bad_params", "{got:?}");
    assert_eq!(
        got[4]["error"]["data"]["field"], "venue",
        "别的平台的管不了"
    );
    assert_eq!(got[5]["error"]["data"]["reason"], "bad_params", "{got:?}");
    assert_eq!(got[5]["error"]["data"]["field"], "on", "{got:?}");
    let groups = dir(&home, &bot(), &session);
    until_turns_in(&groups, 1).await;
    std::fs::write(&first_done, "").expect("写得进");
    let got = replies(&path, 9).await;
    assert_eq!(got[7]["result"], json!({}), "{got:?}");
    until_turns_in(&groups, 2).await;

    let requests = script.requests();
    let first = names(&requests[0].1);
    assert!(
        first.contains(&"qq_send".to_string()) && !first.contains(&"qq_mute".to_string()),
        "没说开的不给，没写的照旧：{first:?}"
    );
    let second = names(&requests[1].1);
    assert!(
        second.contains(&"qq_send".to_string()) && second.contains(&"qq_mute".to_string()),
        "说开了下一轮给：{second:?}"
    );

    // 本机的头不是系统账号。
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let refused = client
        .call(
            "p1",
            "venue.plugins",
            json!({"venue": "qq:group:1", "on": []}),
        )
        .await;
    assert_eq!(reason(&refused), Some("no_system_account"), "{refused}");
    core.stop_extensions().await;
    core.stop_sessions().await;
}
