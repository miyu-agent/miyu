//! 补拉回来的群消息（施工 O-40，`docs/blueprint/venues.md`「场所的格」）：`venue.backfill`、`venue.sent` 原样记；补拉的要带着
//! `ambient`，平台的时刻要合写法，不合的 `bad_params`，什么都不记。

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

/// 代表主人说一句，场所的格是 `venue`。
fn said(session: &str, venue: Value) -> Value {
    json!({"session": session, "text": "断线时说的", "as": {"external": "qq:10001"}, "venue": venue})
}

#[tokio::test]
async fn a_backfilled_message_is_kept_with_its_platform_time() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let venue = json!({"msg": "8799", "ambient": true, "backfill": true, "sent": "2026-10-20T04:30:00.000Z"});
    let reply = client
        .call("s1", "session.send", said(&session, venue))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let log = home.log(&session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    let written =
        serde_json::to_value(message.venue.as_ref().expect("有场所的格")).expect("写得成");
    assert_eq!(
        written,
        json!({"msg": "8799", "ambient": true, "backfill": true, "sent": "2026-10-20T04:30:00.000Z"})
    );
}

#[tokio::test]
async fn backfill_without_ambient_or_a_bad_time_is_refused() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    for (id, venue) in [
        ("b1", json!({"msg": "8799", "backfill": true})),
        (
            "b2",
            json!({"msg": "8799", "ambient": true, "sent": "2026-10-20 04:30"}),
        ),
    ] {
        let reply = client.call(id, "session.send", said(&session, venue)).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都不记");
}
