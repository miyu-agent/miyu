//! 给头看的结构化结果（施工 W-11，2026-10-17 两个头要的）：真核心、真的记忆工具。`memory_search` 交的 `detail` 原样记进
//! `tool.result.detail`，视图的工具卡带上它；不进给模型的请求，她看到的只有结果的字。

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

#[tokio::test]
async fn a_tool_detail_is_logged_for_heads_and_kept_from_the_model() {
    let home = Home::new();
    let tools =
        Catalog::new(miyu_memory::tools(&default_resources()).expect("读得出")).expect("合写法");
    let script = Script::new([
        Play::calls(&[("remember", r#"{"class":"user","text":"用户养了一只猫"}"#)]),
        Play::calls(&[("memory_search", r#"{"query":"猫"}"#)]),
        Play::Says("记得。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你记得我养什么吗").await;
    home.until_turns(&session, 1).await;

    let details: Vec<String> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => result.detail.map(|detail| detail.get().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(details.len(), 1, "只有搜的那一件交了：{details:?}");
    let detail: serde_json::Value = serde_json::from_str(&details[0]).expect("是 JSON");
    assert_eq!(detail["memories"][0]["text"], "用户养了一只猫", "{detail}");
    assert_eq!(detail["memories"][0]["id"], "m1");

    let requests = script.requests();
    let last = format!("{:?}", requests.last().expect("请求过").1);
    assert!(!last.contains("\"memories\""), "detail 不给她看：{last}");

    let page = client
        .call("v1", "view.page", json!({"session": session, "view": true}))
        .await;
    let text = page.to_string();
    assert!(
        text.contains("\"detail\":{\"memories\""),
        "视图的工具卡带上它：{page}"
    );
}
