//! 这时的上下文一块块的（施工 CX-1 上，`session.context`）：真核心、出厂的基础系统。造好的会话有 system、内置的工具，
//! 没说过话的没有对话那块；说过一轮的有人说的、她说的；名字照连接的语言；写了别的参数、没有的会话拒。

use std::path::Path;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

fn part<'a>(parts: &'a Value, id: &str) -> Option<&'a Value> {
    parts.as_array()?.iter().find(|part| part["part"] == id)
}

#[tokio::test]
async fn a_session_tells_its_context_part_by_part() {
    let home = Home::new();
    let mut client =
        Client::connect(home.core_with_tools(&Script::new([Play::Says("好的。")]), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;

    let fresh = client
        .call("x1", "session.context", json!({"session": session}))
        .await;
    let result = &fresh["result"];
    let parts = &result["parts"];
    let system = part(parts, "system").unwrap_or_else(|| panic!("有 system：{fresh}"));
    assert!(system["tokens"].as_u64() > Some(0), "{fresh}");
    let other = part(&system["children"], "system.other").expect("有其余的");
    assert_ne!(other["label"], "system.other", "名字照语言：{fresh}");
    let tools = part(parts, "tools").unwrap_or_else(|| panic!("有工具：{fresh}"));
    let builtin = part(&tools["children"], "tools.builtin").expect("有内置的");
    assert_eq!(builtin["tokens"], tools["tokens"], "全是内置的：{fresh}");
    assert!(part(parts, "messages").is_none(), "没说过话：{fresh}");
    let total: u64 = parts
        .as_array()
        .unwrap()
        .iter()
        .map(|part| part["tokens"].as_u64().unwrap())
        .sum();
    assert_eq!(result["estimated_total"], total, "{fresh}");
    assert_eq!(result["basis"], "estimated", "还没答过：{fresh}");

    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let spoken = client
        .call("x2", "session.context", json!({"session": session}))
        .await;
    let messages =
        part(&spoken["result"]["parts"], "messages").unwrap_or_else(|| panic!("有对话：{spoken}"));
    for said in ["messages.user", "messages.assistant"] {
        let found = part(&messages["children"], said).unwrap_or_else(|| panic!("{said}：{spoken}"));
        assert!(found["tokens"].as_u64() > Some(0), "{spoken}");
    }
}

#[tokio::test]
async fn other_params_or_an_unknown_session_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    for (id, params, expected) in [
        ("r1", json!({}), "bad_params"),
        (
            "r2",
            json!({"session": session, "preset": "x"}),
            "bad_params",
        ),
        (
            "r3",
            json!({"session": "0192f3a0-1111-7abc-8def-001122334455"}),
            "session_not_found",
        ),
        ("r4", json!({"session": "nope"}), "bad_params"),
    ] {
        let reply = client.call(id, "session.context", params).await;
        assert_eq!(reason(&reply), Some(expected), "{reply}");
    }
}
