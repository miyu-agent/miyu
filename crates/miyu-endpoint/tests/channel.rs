//! 她知道这一轮的话从哪来（施工 VN-1）：人在头上说的话记上握手报的头的种类（`by.head`），回合开始的事实里多一块出厂的那一句；
//! 同一个地方接着说不再加，换一个头说才加。写法不对的种类不记。

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 出厂的那一句。
fn line(file: &str) -> String {
    std::fs::read_to_string(default_resources().join("core/facts/channel").join(file))
        .expect("出厂的有")
}

/// 握手报头的种类是 `kind`。
async fn hello_as_head(client: &mut Client, kind: &str) -> Value {
    client
        .call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": kind, "version": "0.0.0"},
                "locale": "zh-CN",
                "caps": {"input": true},
                "token": TOKEN,
            }),
        )
        .await
}

/// 日志里人说的话的 `by`。
fn speakers(log: &[Event]) -> Vec<Value> {
    log.iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| json!(event.by))
        .collect()
}

/// 日志里内核记的这一类的几块。
fn told(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "channel" => {
                Some(fact.text.clone())
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn she_is_told_which_head_the_words_came_from_when_it_changes() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("在。")]);
    let core = home.core(&script);
    let mut web = Client::connect(core.clone());
    hello_as_head(&mut web, "web").await;
    let session = web.create("c1", "~").await;
    web.say("c2", &session, "你在哪？").await;
    home.until_turns(&session, 1).await;
    web.say("c3", &session, "还在吗？").await;
    home.until_turns(&session, 2).await;
    let mut tui = Client::connect(core);
    hello_as_head(&mut tui, "tui").await;
    tui.say("c4", &session, "换个地方。").await;
    home.until_turns(&session, 3).await;
    let log = home.log(&session);
    let web_by = json!({"kind": "person", "head": "web"});
    let tui_by = json!({"kind": "person", "head": "tui"});
    assert_eq!(speakers(&log), [web_by.clone(), web_by, tui_by]);
    assert_eq!(
        told(&log),
        [line("head/web.txt"), line("head/tui.txt")],
        "同一个头接着说不再加"
    );
    let requests = script.requests();
    let Some(Message::User { blocks }) = requests[0].1.messages.last() else {
        panic!("最后一条是人这边的");
    };
    let text = format!("{blocks:?}");
    assert!(
        text.contains(line("head/web.txt").trim_end()),
        "她收到了：{text}"
    );
}

#[tokio::test]
async fn an_unknown_or_badly_written_kind_is_another_app() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    hello_as_head(&mut client, "Not A Name").await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    assert_eq!(
        speakers(&log),
        [json!({"kind": "person"})],
        "写法不对的不记"
    );
    assert_eq!(told(&log), [line("other.txt")]);
}
