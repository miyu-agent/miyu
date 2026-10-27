//! 场所消息里这个群叫什么（施工 E-4 上，`docs/construction/E-4上-记下在哪个群.md`）：`venue.place` 同 `name` 的规矩，原样记进
//! `message.user` 的 `venue`，没写的不写；空白、超长、带控制字符、不是字的 `bad_params`，什么都不记。核心不看是群还是私聊（私聊
//! 桥不写）。群名是编的。

use serde_json::{Value, json};

use miyu_kernel::event::{Body, VenueMessage};
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊，交回连接和会话编号。
async fn chat(home: &Home) -> (Client, String) {
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

/// 主人旁听地说一句，场所的格另带 `place`（`Value::Null` 的不写）。
fn said(session: &str, msg: &str, place: Value) -> Value {
    let mut venue = json!({"msg": msg, "name": "阿楠", "ambient": true});
    if !place.is_null() {
        venue["place"] = place;
    }
    json!({"session": session, "text": "在吗", "as": {"external": "qq:10001"}, "venue": venue})
}

/// 会话日志最后一条人话的场所的格。
fn last_venue(home: &Home, session: &str) -> Value {
    let log = home.log(session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    serde_json::to_value(message.venue.as_ref().expect("有场所的格")).expect("写得成")
}

#[tokio::test]
async fn place_is_kept_as_written_and_absent_is_not_written() {
    let home = Home::new();
    let (mut client, session) = chat(&home).await;
    let reply = client
        .call(
            "s1",
            "session.send",
            said(&session, "8801", json!(" 周末摄影群 ")),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        last_venue(&home, &session)["place"],
        json!(" 周末摄影群 "),
        "原样记"
    );
    let reply = client
        .call("s2", "session.send", said(&session, "8802", Value::Null))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(
        last_venue(&home, &session).get("place").is_none(),
        "没写的不写"
    );
    // 以前记的（日志里没有这一格）照样读得进。
    let older: VenueMessage = serde_json::from_value(json!({"msg": "8800"})).expect("读得进");
    assert_eq!(older.place, None);
}

#[tokio::test]
async fn a_bad_place_records_nothing() {
    let home = Home::new();
    let (mut client, session) = chat(&home).await;
    let before = home.log(&session).len();
    for (id, bad) in [
        ("b1", json!("   ")),
        ("b2", json!("群".repeat(65))),
        ("b3", json!("周末\n摄影群")),
        ("b4", json!(7)),
    ] {
        let reply = client
            .call(id, "session.send", said(&session, "8803", bad))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{id}: {reply}");
    }
    let fits = client
        .call(
            "ok",
            "session.send",
            said(&session, "8804", json!("群".repeat(64))),
        )
        .await;
    assert!(fits.get("error").is_none(), "64 个字正好：{fits}");
    assert_eq!(home.log(&session).len(), before + 1, "写错的什么都没记");
}
