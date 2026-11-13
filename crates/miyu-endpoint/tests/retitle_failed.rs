//! 辅助请求的总时限和人要求的重起标题没起成（施工 TT-3，`docs/blueprint/kernel/session.md`「起标题」第 11 条、
//! `session/actor.md` 第 8 条）：起标题、回顾从交给端口起最多等总时限（出厂 180 秒，这里的剧本设短的），到时没说完照出错了结；
//! 人要的重起超时、出错，事件流推 `session.retitle_failed`（`by` 是要它的人）、视图流出一条 `retitle_failed` 旁白（字照连接的
//! 语言），标题不动；起成了的、没超时的照常，不推。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, CallResult, ErrorClass, Purpose};
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 测试里的总时限：不用真等 180 秒。
const LIMIT: Duration = Duration::from_millis(300);

/// 造一个人起过名「我起的」、说过一轮的会话，事件流订阅着；另一个头订阅它的视图流。交回会话编号和看视图流的头。
async fn spoken(
    client: &mut Client,
    core: &std::sync::Arc<miyu_endpoint::Core>,
) -> (String, Client) {
    client.hello().await;
    let session = client.create("c1", "~").await;
    let named = client
        .call(
            "c2",
            "session.set_meta",
            json!({"session": session, "title": "我起的"}),
        )
        .await;
    assert_eq!(named["result"], json!({}), "{named}");
    client.subscribe("s1", &session).await;
    client.say("c3", &session, "hi").await;
    client.until_turn_ends(&session).await;
    let mut viewer = Client::connect(std::sync::Arc::clone(core));
    viewer.hello().await;
    let params = json!({"session": session, "stream": "view"});
    let reply = viewer.call("v1", "subscribe", params).await;
    assert!(reply["result"].is_object(), "{reply}");
    (session, viewer)
}

/// 要一次重起，回应得是 `{}`。
async fn retitle(client: &mut Client, id: &str, session: &str) {
    let reply = client
        .call(id, "session.retitle", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

/// 读事件流，读到会话 `session` 的 `session.retitle_failed` 或者写了标题的 `session.meta_changed` 为止，交回那一条。
async fn settled_title(client: &mut Client, session: &str) -> Value {
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] != json!("event") || next["params"]["session"] != json!(session) {
            continue;
        }
        let event = next["params"]["event"].clone();
        if event["kind"] == json!("session.retitle_failed")
            || event["kind"] == json!("session.meta_changed")
        {
            return event;
        }
    }
}

/// 读视图流，读到 `retitle_failed` 那一条旁白为止。
async fn notice(viewer: &mut Client) -> Value {
    loop {
        let next = viewer.next().await.expect("没断开");
        let entry = &next["params"]["entry"];
        if next["method"] == json!("view.add") && entry["what"] == json!("retitle_failed") {
            return entry.clone();
        }
    }
}

/// 会话列表里会话 `session` 的标题。
async fn title(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .map(|item| item["title"].clone())
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

#[tokio::test]
async fn a_retitle_past_the_limit_is_stopped_told_and_keeps_the_title() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")])
        .titles([Play::Holds])
        .aside_limit(LIMIT);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    let (session, mut viewer) = spoken(&mut client, &core).await;
    retitle(&mut client, "c4", &session).await;
    let failed = settled_title(&mut client, &session).await;
    assert_eq!(failed["kind"], json!("session.retitle_failed"), "{failed}");
    assert_eq!(failed["by"], json!({"kind": "person"}), "{failed}");
    assert_eq!(failed["body"]["error"]["class"], json!("retryable"));
    assert_eq!(
        failed["body"]["error"]["message"],
        json!("总超时：0.3 秒还没有说完")
    );
    assert_eq!(
        failed["body"]["human"],
        json!({"key": "core/sessions/retitle-failed"})
    );
    let entry = notice(&mut viewer).await;
    assert_eq!(entry["text"], json!("更改标题失败"), "{entry}");
    // 到时叫停了端口：剧本里那一次起标题的请求被叫停。
    let titled = script.titled();
    let [(seen, _)] = titled.as_slice() else {
        panic!("一次起标题的请求：{}", titled.len());
    };
    assert!(
        script.cancelled().contains(seen),
        "{:?}",
        script.cancelled()
    );
    assert_eq!(title(&mut client, "c5", &session).await, json!("我起的"));
    // 记账的那一条照出错记。
    let called = home
        .log(&session)
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Title) => Some(called),
            _ => None,
        })
        .expect("有起标题的那一条");
    assert_eq!(called.result, CallResult::Error);
}

#[tokio::test]
async fn a_failed_retitle_is_told_and_one_in_time_is_not() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")])
        .titles([
            Play::Fails {
                class: ErrorClass::Retryable,
                wait_ms: None,
            },
            Play::Says("模型起的"),
        ])
        .aside_limit(LIMIT);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    let (session, _viewer) = spoken(&mut client, &core).await;
    retitle(&mut client, "c4", &session).await;
    let failed = settled_title(&mut client, &session).await;
    assert_eq!(failed["kind"], json!("session.retitle_failed"), "{failed}");
    assert_eq!(
        failed["body"]["error"]["message"],
        json!("HTTP 429: slow down")
    );
    assert_eq!(title(&mut client, "c5", &session).await, json!("我起的"));
    retitle(&mut client, "c6", &session).await;
    let renamed = settled_title(&mut client, &session).await;
    assert_eq!(renamed["body"], json!({"title": "模型起的"}), "{renamed}");
    // 过了总时限，说完了的那一次不再当超时。
    tokio::time::sleep(LIMIT * 2).await;
    while let Some(next) = client.next_within(Duration::from_millis(100)).await {
        assert_ne!(
            next["params"]["event"]["kind"],
            json!("session.retitle_failed"),
            "{next}"
        );
    }
    assert_eq!(title(&mut client, "c7", &session).await, json!("模型起的"));
}

#[tokio::test]
async fn an_automatic_title_past_the_limit_is_not_told() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又好。")])
        .titles([Play::Holds, Play::Says("自动的")])
        .aside_limit(LIMIT);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("s1", &session).await;
    client.say("c2", &session, "hi").await;
    client.until_turn_ends(&session).await;
    tokio::time::sleep(LIMIT * 2).await;
    // 自动起的第一次超时了不推；它照失败记，下一轮答完再试一次，起成了。
    client.say("c3", &session, "again").await;
    let renamed = settled_title(&mut client, &session).await;
    assert_eq!(renamed["kind"], json!("session.meta_changed"), "{renamed}");
    assert_eq!(renamed["body"], json!({"title": "自动的"}));
    let titled = script.titled();
    assert_eq!(titled.len(), 2);
    assert!(script.cancelled().contains(&titled[0].0));
}

#[tokio::test]
async fn a_recap_past_the_limit_is_refused() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Holds]).aside_limit(LIMIT);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    client.subscribe("s1", &session).await;
    client.say("c2", &session, "hi").await;
    client.until_turn_ends(&session).await;
    let reply = client
        .call("c3", "session.recap", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("recap_failed"), "{reply}");
}

/// 模型起标题时要了工具（主人本机 2026-11-13：网关把它换成一句占位的字一起交回来，那句字被当成了标题）：标题不动，人要的推失败。
#[tokio::test]
async fn a_title_reply_that_asks_for_a_tool_is_a_failure() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).titles([Play::SaysThenCalls {
        text: "(The model asked for a tool this session doesn't have)",
        calls: vec![("read".to_string(), "{}".to_string())],
    }]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    let (session, _viewer) = spoken(&mut client, &core).await;
    retitle(&mut client, "c4", &session).await;
    let failed = settled_title(&mut client, &session).await;
    assert_eq!(failed["kind"], json!("session.retitle_failed"), "{failed}");
    assert_eq!(failed["body"]["error"]["class"], json!("empty_reply"));
    assert_eq!(title(&mut client, "c5", &session).await, json!("我起的"));
}
