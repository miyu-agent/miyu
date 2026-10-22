//! 无工具（施工 P-16 下，`docs/blueprint/presets.md`「无工具」）：真核心走一遍。`preset.list` 末尾固定一项编号是 `null` 的
//! 「无工具」；开会话交 `preset: null` 的一件工具都不给，会话信息写 `preset: null`、`no_tools: true`；中途换成它、再换回来；
//! 交空字的照旧拒。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::preset_face::{connected, tool_names};
use crate::support::venues::configured_core;
use crate::support::*;

/// 基础系统两件。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([("basesystem", vec![fake("read"), fake("shell")])]).expect("合写法")
}

fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, catalog())
}

/// 造一个会话，交回编号。
async fn create(client: &mut Client, params: Value) -> String {
    let reply = client.call("c1", "session.create", params).await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string()
}

/// 会话列表里的这一项。
async fn listed(client: &mut Client, session: &str) -> Value {
    let reply = client.call("l1", "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["session"] == session).cloned())
        .unwrap_or_else(|| panic!("列表里没有：{reply}"))
}

#[tokio::test]
async fn the_list_ends_with_no_tools_in_the_connection_s_language() {
    let home = Home::new();
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let reply = client.call("p1", "preset.list", json!({})).await;
    let last = reply["result"]["presets"]
        .as_array()
        .and_then(|all| all.last().cloned())
        .unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(
        last,
        json!({"preset": null, "name": "无工具", "summary": "不提供任何工具。", "icon": "circle-off"})
    );
}

#[tokio::test]
async fn a_session_opened_with_no_tools_gets_none_and_says_so() {
    let home = Home::new();
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured(&home, &script)).await;
    let session = create(&mut client, json!({"cwd": "~", "preset": null})).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    assert!(
        tool_names(&request).is_empty(),
        "{:?}",
        tool_names(&request)
    );
    let item = listed(&mut client, &session).await;
    assert_eq!(
        (&item["preset"], &item["no_tools"]),
        (&Value::Null, &json!(true)),
        "{item}"
    );
    let created = crate::preset_face::snapshot(&home, &session);
    assert!(
        created.no_tools && created.preset.is_none(),
        "快照记了无工具"
    );
    // 核心重启、会话重新载入以后的第一轮照快照重新筛：照旧一件都不给，不当成以前没记预设的（那种不筛、全给）。
    drop(client);
    let again = Script::new([Play::Says("又来。")]);
    let mut client = connected(configured(&home, &again)).await;
    client.say("s2", &session, "hi").await;
    home.until_turns(&session, 2).await;
    let (_, request) = again.requests().into_iter().next().expect("重启后发了请求");
    assert!(
        tool_names(&request).is_empty(),
        "{:?}",
        tool_names(&request)
    );
}

#[tokio::test]
async fn a_session_switches_to_no_tools_and_back() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = connected(configured(&home, &script)).await;
    let session = create(&mut client, json!({"cwd": "~"})).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let to_none = client
        .call(
            "k1",
            "session.configure",
            json!({"session": session, "preset": null}),
        )
        .await;
    assert!(to_none.get("error").is_none(), "{to_none}");
    client.say("s2", &session, "hi").await;
    home.until_turns(&session, 2).await;
    let item = listed(&mut client, &session).await;
    assert_eq!(
        (&item["preset"], &item["no_tools"]),
        (&Value::Null, &json!(true)),
        "{item}"
    );
    let back = client
        .call(
            "k2",
            "session.configure",
            json!({"session": session, "preset": "full"}),
        )
        .await;
    assert!(back.get("error").is_none(), "{back}");
    client.say("s3", &session, "hi").await;
    home.until_turns(&session, 3).await;
    let faces: Vec<Vec<String>> = script
        .requests()
        .iter()
        .map(|(_, request)| {
            tool_names(request)
                .into_iter()
                .map(str::to_string)
                .collect()
        })
        .collect();
    assert_eq!(
        faces,
        [vec!["read", "shell"], vec![], vec!["read", "shell"]]
    );
    let item = listed(&mut client, &session).await;
    assert_eq!(
        (&item["preset"], item.get("no_tools")),
        (&json!("full"), None),
        "{item}"
    );
}

#[tokio::test]
async fn an_empty_preset_is_still_refused() {
    let home = Home::new();
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": "~", "preset": ""}))
        .await;
    assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");
}
