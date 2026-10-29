//! 转发里带的东西的编号（施工 O-61，`docs/blueprint/venues.md`「场所的格」）：`venue.forwards` 里每样可选的 `index`（正整数）
//! 原样记，没写的不写；写在 `venue.media` 里的、写 0 的、不是整数的整条 `bad_params`，什么都不记。

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

/// 代表主人旁听地交一条带转发的消息，场所的格另带 `media`、转发里那一条带 `forwarded`。
fn sent(session: &str, media: Value, forwarded: Value) -> Value {
    json!({"session": session, "text": "[forward]", "as": {"external": "qq:10001"},
        "venue": {"msg": "8810", "ambient": true, "media": media,
            "forwards": [{"nodes": [{"name": "阿杰", "text": "", "media": forwarded}]}]}})
}

#[tokio::test]
async fn an_index_in_a_forward_is_kept_as_written() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let forwarded = json!([
        {"kind": "image", "id": "i-1", "index": 2},
        {"kind": "file", "id": "f-1", "name": "a.pdf"},
    ]);
    let reply = client
        .call("s1", "session.send", sent(&session, json!([]), forwarded))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let log = home.log(&session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    let written = serde_json::to_value(message.venue.as_ref().expect("有场所的格")).unwrap();
    assert_eq!(
        written["forwards"][0]["nodes"][0]["media"],
        json!([
            {"kind": "image", "id": "i-1", "index": 2},
            {"kind": "file", "id": "f-1", "name": "a.pdf"},
        ]),
        "写了的原样记，没写的不写"
    );
}

#[tokio::test]
async fn an_index_outside_a_forward_or_not_positive_records_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    let image = |index: Value| json!([{"kind": "image", "id": "i-1", "index": index}]);
    let outside = sent(&session, image(json!(1)), json!([]));
    let reply = client.call("b1", "session.send", outside).await;
    assert_eq!(
        reason(&reply),
        Some("bad_params"),
        "只在转发里有意义：{reply}"
    );
    for (id, index) in [("b2", json!(0)), ("b3", json!(-1)), ("b4", json!("1"))] {
        let reply = client
            .call(id, "session.send", sent(&session, json!([]), image(index)))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}
