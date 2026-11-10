//! 人要求重起标题（施工 TT-1，`docs/blueprint/protocol.md` 的 `session.retitle`）：照自动起标题的那一条路再发一次请求，
//! 喂的是最近三轮（自动起的照旧只喂第一轮），
//! 请求发出去就回 `{}`，标题经 `session.meta_changed` 推来（`by` 是内核，不算人起的）；人起过名的也换掉；说了不自动起的
//! 照样起；一次在路上时再要的不重复发；没东西可起的拒。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 读推送，读到会话 `session` 的 `session.meta_changed` 为止，交回读到的事件，照先后。
async fn until_renamed(client: &mut Client, session: &str) -> Vec<Value> {
    let mut events = Vec::new();
    loop {
        let next = client.next().await.expect("没断开");
        if next["method"] != json!("event") || next["params"]["session"] != json!(session) {
            continue;
        }
        let event = next["params"]["event"].clone();
        let done = event["kind"] == json!("session.meta_changed");
        events.push(event);
        if done {
            return events;
        }
    }
}

/// 等会话 `session` 的执行器把手上的一批动作做完：发一个什么都不改的命令，它回了，之前那一批一定已经交给了端口。
async fn settled(client: &mut Client, id: &str, session: &str) {
    let reply = client
        .call(
            id,
            "session.set_meta",
            json!({"session": session, "pinned": false}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

/// `session.list` 里会话 `session` 那一项。
async fn listed(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

/// 要一次重起，回应得是 `{}`。
async fn retitle(client: &mut Client, id: &str, session: &str) {
    let reply = client
        .call(id, "session.retitle", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

/// 一次起标题的请求里那一条 user 的字。
fn asked(request: &Request) -> &str {
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    &text.text
}

#[tokio::test]
async fn retitling_feeds_the_latest_turns_and_pushes_the_new_title() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("答一"),
        Play::Says("答二"),
        Play::Says("答三"),
        Play::Says("答四"),
    ])
    .titles([Play::Says("打个招呼"), Play::Says("换一个")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("s1", &session).await;
    client.say("c2", &session, "一").await;
    until_renamed(&mut client, &session).await;
    for (id, words) in [("c3", "二"), ("c4", "三"), ("c5", "四")] {
        client.say(id, &session, words).await;
        client.until_turn_ends(&session).await;
    }
    retitle(&mut client, "c6", &session).await;
    let events = until_renamed(&mut client, &session).await;
    let [.., called, renamed] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(called["kind"], json!("model.called"));
    assert_eq!(called["body"]["purpose"], json!("title"));
    assert_eq!(renamed["body"], json!({"title": "换一个"}));
    assert_eq!(renamed["by"], json!({"kind": "kernel"}), "{renamed}");
    let titled = script.titled();
    let [(_, auto), (_, again)] = titled.as_slice() else {
        panic!("两次起标题的请求：{}", titled.len());
    };
    // 自动起的只喂第一轮；人要的喂最近三轮，没有第一轮。指令是同一份，别的格一样。
    let (auto_text, again_text) = (asked(auto), asked(again));
    let (instruction, first) = auto_text
        .split_once("Conversation:\n")
        .expect("指令的最后一行");
    assert_eq!(first, "User: 一\n\nAssistant: 答一");
    assert_eq!(
        again_text.strip_prefix(instruction),
        Some(
            "Conversation:\nUser: 二\n\nAssistant: 答二\n\nUser: 三\n\nAssistant: 答三\n\nUser: 四\n\nAssistant: 答四"
        )
    );
    assert!(again.tools.is_empty() && again.system.is_empty());
    let item = listed(&mut client, "c7", &session).await;
    assert_eq!(item["title"], json!("换一个"));
    assert!(item.get("named").is_none(), "不算人起的：{item}");
}

#[tokio::test]
async fn retitling_replaces_a_title_a_person_gave() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("模型起的")]);
    let mut client = Client::connect(home.core(&script));
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
    settled(&mut client, "c4", &session).await;
    assert!(script.titled().is_empty(), "人起过名的不自动起");
    assert_eq!(
        listed(&mut client, "c5", &session).await["named"],
        json!(true)
    );
    retitle(&mut client, "c6", &session).await;
    let events = until_renamed(&mut client, &session).await;
    assert_eq!(
        events.last().expect("有一条")["body"],
        json!({"title": "模型起的"})
    );
    let item = listed(&mut client, "c7", &session).await;
    assert_eq!(item["title"], json!("模型起的"));
    assert!(item.get("named").is_none(), "换成了不是人起的：{item}");
}

#[tokio::test]
async fn a_session_with_nothing_said_is_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "session.retitle", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert!(script.titled().is_empty());
    let missing = client.call("c3", "session.retitle", json!({})).await;
    assert_eq!(reason(&missing), Some("bad_params"), "{missing}");
}

#[tokio::test]
async fn a_second_ask_while_one_is_on_its_way_sends_nothing_more() {
    let home = Home::new();
    // 起标题没排剧本：请求记下、不回，一直在路上。
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    client.subscribe("s1", &session).await;
    client.say("c2", &session, "hi").await;
    client.until_turn_ends(&session).await;
    settled(&mut client, "c3", &session).await;
    assert!(script.titled().is_empty(), "说了不自动起的不起");
    retitle(&mut client, "c4", &session).await;
    retitle(&mut client, "c5", &session).await;
    settled(&mut client, "c6", &session).await;
    assert_eq!(script.titled().len(), 1, "只发一次");
}

#[tokio::test]
async fn a_session_that_does_not_title_itself_is_still_retitled_when_asked() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("要了才起")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    client.subscribe("s1", &session).await;
    client.say("c2", &session, "hi").await;
    client.until_turn_ends(&session).await;
    retitle(&mut client, "c3", &session).await;
    let events = until_renamed(&mut client, &session).await;
    assert_eq!(
        events.last().expect("有一条")["body"],
        json!({"title": "要了才起"})
    );
}
