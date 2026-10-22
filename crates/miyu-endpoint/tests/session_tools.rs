//! 下一次请求会发的工具（施工 S-T，`session.tools`）：真核心、出厂的基础系统。写了会话的问那个会话，照名字排、带来处；
//! 不写会话的照新会话会拿到的算，和真造出来的那个会话的一样；会话、人格预设一起写的拒，没有的会话拒。

use std::path::Path;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::*;

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

fn names(reply: &Value) -> Vec<String> {
    reply["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("有 tools：{reply}"))
        .iter()
        .map(|tool| tool["name"].as_str().expect("有名字").to_string())
        .collect()
}

#[tokio::test]
async fn a_session_and_a_new_one_list_the_same_tools_in_sent_order() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;

    let reply = client
        .call("t1", "session.tools", json!({"session": session}))
        .await;
    let listed = names(&reply);
    assert!(listed.iter().any(|name| name == "shell"), "{reply}");
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted, "照发的先后：名字的字节序");
    let shell = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "shell")
        .cloned()
        .unwrap();
    assert_eq!(shell["source"]["kind"], "builtin", "{shell}");
    assert_eq!(shell["source"]["id"], "basesystem", "{shell}");
    assert!(shell.get("loaded").is_none(), "不用加载的不写：{shell}");

    let preview = client.call("t2", "session.tools", json!({})).await;
    assert_eq!(names(&preview), listed, "新会话会拿到的和真造出来的一样");
}

#[tokio::test]
async fn mixing_a_session_with_a_persona_or_an_unknown_session_is_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    for (id, params, expected) in [
        (
            "r1",
            json!({"session": session, "preset": "x"}),
            "bad_params",
        ),
        (
            "r2",
            json!({"session": session, "persona": null}),
            "bad_params",
        ),
        (
            "r3",
            json!({"session": "0192f3a0-1111-7abc-8def-001122334455"}),
            "session_not_found",
        ),
        ("r4", json!({"session": "nope"}), "bad_params"),
    ] {
        let reply = client.call(id, "session.tools", params).await;
        assert_eq!(reason(&reply), Some(expected), "{reply}");
    }
}
