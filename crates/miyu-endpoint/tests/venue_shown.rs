//! 场所消息里交了原图的那几样（施工 O-35，`docs/construction/O-35-私聊看文件.md`）：`venue.media` 每项可选的 `shown` 原样记，
//! 假的、没写的不写；带的东西全是交了原图的、又没有正文也没附件的 `empty_message`；转发里的写了 `shown`、写的不是真假的
//! `bad_params`；都什么都不记。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊，交回连接和会话编号。
async fn owners_chat(home: &Home) -> (Client, String) {
    let mut client = Client::connect(bound_core(home, &Script::new([]), Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{made}"))
        .to_string();
    (client, session)
}

/// 代表主人旁听地说一句，`text` 是正文，场所的格另带 `media`。
fn said(session: &str, text: &str, media: Value) -> Value {
    json!({"session": session, "text": text, "as": {"external": "qq:10001"},
        "venue": {"msg": "8810", "ambient": true, "media": media}})
}

#[tokio::test]
async fn shown_is_kept_as_written_and_false_is_not_written() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let media = json!([
        {"kind": "image", "id": "i-1", "size": 2048, "shown": true},
        {"kind": "file", "id": "f-1", "name": "排班.pdf", "shown": false},
        {"kind": "sticker", "id": "s-1"},
    ]);
    let reply = client
        .call("s1", "session.send", said(&session, "看", media))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let log = home.log(&session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    let written = serde_json::to_value(message.venue.as_ref().expect("有场所的格")).unwrap();
    assert_eq!(
        written["media"],
        json!([
            {"kind": "image", "id": "i-1", "size": 2048, "shown": true},
            {"kind": "file", "id": "f-1", "name": "排班.pdf"},
            {"kind": "sticker", "id": "s-1"},
        ]),
        "真的原样记，假的不写"
    );
}

#[tokio::test]
async fn all_shown_without_blocks_is_empty_and_bad_shown_records_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    let shown = json!([{"kind": "image", "id": "i-1", "shown": true}]);
    let reply = client
        .call("e1", "session.send", said(&session, "", shown))
        .await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
    let forwarded = json!({"session": session, "text": "[forward]", "as": {"external": "qq:10001"},
        "venue": {"msg": "8811", "ambient": true, "forwards": [{"nodes": [
            {"name": "阿杰", "text": "", "media": [{"kind": "image", "id": "i-2", "shown": true}]}]}]}});
    let reply = client.call("b1", "session.send", forwarded).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let not_bool = json!([{"kind": "image", "id": "i-1", "shown": "yes"}]);
    let reply = client
        .call("b2", "session.send", said(&session, "看", not_bool))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}
