//! 调工具：几种内容照原样读回来；工具自己说出错了不算出错；不认识的工具是服务的错；要补信息的不支持，只带 requestState 的照着重发；服务反过来问的回
//! 「没有这个方法」再接着答；等不及丢掉的发取消；服务退出了是断了。

use std::time::Duration;

use serde_json::json;

use miyu_mcp::testkit::{Kind, Script};
use miyu_mcp::{Content, Failed};

use crate::support::connected;

#[tokio::test]
async fn content_comes_back_as_it_was_sent() {
    for script in [Script::modern(), Script::legacy(Kind::LegacyRefuses)] {
        let (client, _) = connected(script).await;
        let echoed = client
            .call("echo", json!({"text": "你好"}))
            .await
            .expect("答得上");
        assert_eq!(echoed.content, [Content::Text("你好".to_string())]);
        assert!(!echoed.is_error);
        let picture = client.call("picture", json!({})).await.expect("答得上");
        assert_eq!(
            picture.content,
            [
                Content::Image {
                    data: "aGk=".to_string(),
                    mime: "image/png".to_string(),
                },
                Content::Text("a picture".to_string()),
            ]
        );
        let structured = client.call("structured", json!({})).await.expect("答得上");
        assert_eq!(structured.structured, Some(json!({"answer": 42})));
    }
}

#[tokio::test]
async fn a_tool_error_is_a_result_and_an_unknown_tool_is_a_server_error() {
    let (client, _) = connected(Script::modern()).await;
    let failed = client.call("fail", json!({})).await.expect("答得上");
    assert!(failed.is_error);
    assert_eq!(failed.content, [Content::Text("broke".to_string())]);
    assert!(matches!(
        client.call("nope", json!({})).await,
        Err(Failed::Rpc { code: -32602, .. })
    ));
}

#[tokio::test]
async fn asking_for_more_input_is_not_supported() {
    let (client, _) = connected(Script::modern()).await;
    assert_eq!(
        client.call("input", json!({})).await,
        Err(Failed::InputRequired)
    );
    assert_eq!(
        client.call("elicit", json!({})).await,
        Err(Failed::InputRequired),
        "要补东西的照旧不支持，带着 requestState 也一样"
    );
}

/// 只带 `requestState` 的 `input_required`（施工 PT-1 中，规范 2026-07-28「Multi Round-Trip Requests」）：换个编号、原样带回
/// `requestState` 再发一遍；一直叫重发的到上限停，说清楚。
#[tokio::test]
async fn a_request_state_alone_is_sent_back_with_the_same_request() {
    let (client, fake) = connected(Script::modern()).await;
    let called = client
        .call("busy", json!({"text": "x"}))
        .await
        .expect("重发以后答完");
    assert_eq!(
        called.content,
        [Content::Text("done after retry".to_string())]
    );
    let sent: Vec<_> = fake
        .received()
        .into_iter()
        .filter(|message| message["params"]["name"] == "busy")
        .collect();
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert_ne!(sent[0]["id"], sent[1]["id"], "重发换编号");
    assert!(sent[0]["params"].get("requestState").is_none());
    assert_eq!(sent[1]["params"]["requestState"], "round-2", "原样带回");
    assert_eq!(
        sent[1]["params"]["arguments"],
        json!({"text": "x"}),
        "原请求照旧"
    );
    let empty = client
        .call("busy-empty", json!({}))
        .await
        .expect("空的 inputRequests 当没要东西");
    assert_eq!(
        empty.content,
        [Content::Text("done after retry".to_string())]
    );
    let nagged = client.call("nagging", json!({})).await;
    assert!(
        matches!(&nagged, Err(Failed::Malformed(why)) if why.contains("kept asking")),
        "{nagged:?}"
    );
    let rounds = fake
        .received()
        .into_iter()
        .filter(|message| message["params"]["name"] == "nagging")
        .count();
    assert_eq!(rounds, 9, "头一回加重发 8 回");
}

#[tokio::test]
async fn a_request_from_the_server_is_refused_and_the_call_goes_on() {
    let (client, fake) = connected(Script::legacy(Kind::LegacyRefuses)).await;
    let called = tokio::time::timeout(Duration::from_secs(10), client.call("ask", json!({})))
        .await
        .expect("十秒内答完")
        .expect("答得上");
    assert_eq!(called.content, [Content::Text("asked".to_string())]);
    let refused = fake
        .received()
        .into_iter()
        .find(|message| message["id"] == "s1")
        .expect("回了它的请求");
    assert_eq!(refused["error"]["code"], -32601);
}

#[tokio::test]
async fn giving_up_on_a_call_cancels_it() {
    let (client, fake) = connected(Script::modern()).await;
    let waited =
        tokio::time::timeout(Duration::from_millis(100), client.call("slow", json!({}))).await;
    assert!(waited.is_err(), "slow 不回");
    let call_id = fake
        .received()
        .iter()
        .find(|message| message["method"] == "tools/call")
        .map(|message| message["id"].clone())
        .expect("发了调用");
    let cancelled = async {
        loop {
            let found = fake.received().into_iter().find(|message| {
                message["method"] == "notifications/cancelled"
                    && message["params"]["requestId"] == call_id
            });
            if found.is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), cancelled)
        .await
        .expect("十秒内收到取消");
    let echoed = client
        .call("echo", json!({"text": "还在"}))
        .await
        .expect("连接照常");
    assert_eq!(echoed.content, [Content::Text("还在".to_string())]);
}

#[tokio::test]
async fn a_server_that_exits_closes_the_connection() {
    let (client, _) = connected(Script::modern()).await;
    let died = tokio::time::timeout(Duration::from_secs(10), client.call("die", json!({})))
        .await
        .expect("十秒内认出断了");
    assert_eq!(died, Err(Failed::Closed));
    tokio::time::timeout(Duration::from_secs(10), client.closed())
        .await
        .expect("十秒内认出断了");
    assert_eq!(client.tools().await, Err(Failed::Closed));
}

#[tokio::test]
async fn a_ping_from_the_server_is_answered() {
    let (client, _) = connected(Script::legacy(Kind::LegacyRefuses)).await;
    let called = tokio::time::timeout(Duration::from_secs(10), client.call("pinged", json!({})))
        .await
        .expect("十秒内答完")
        .expect("答得上");
    assert_eq!(called.content, [Content::Text("pong".to_string())]);
}

/// 丢下客户端就关了服务的输入（施工 X-3 上）：核心停服务时先关它的标准输入、等它自己退出。服务的输出还开着也一样。
#[tokio::test]
async fn dropping_the_client_closes_the_server_input() {
    let (client, fake) = connected(Script::modern()).await;
    drop(client);
    tokio::time::timeout(Duration::from_secs(10), async {
        while !fake.ended() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("服务读到头、演完了");
}
