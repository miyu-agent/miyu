//! 场所消息里她叫什么（施工 E-1 修三，`docs/construction/E-1修三-她在场所里叫什么.md`）：`venue.my_name` 同 `name` 的规矩，
//! 原样记进 `message.user` 的 `venue`，没写的不写；空白、超长、带控制字符、不是字的 `bad_params`，什么都不记。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊（私聊里桥也带她的昵称），交回连接和会话编号。
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

/// 主人旁听地说一句，场所的格另带 `my_name`（`Value::Null` 的不写）。
fn said(session: &str, msg: &str, my_name: Value) -> Value {
    let mut venue = json!({"msg": msg, "name": "阿楠", "ambient": true});
    if !my_name.is_null() {
        venue["my_name"] = my_name;
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
async fn my_name_is_kept_as_written_and_absent_is_not_written() {
    let home = Home::new();
    let (mut client, session) = chat(&home).await;
    let reply = client
        .call(
            "s1",
            "session.send",
            said(&session, "8801", json!(" 米羽 ")),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        last_venue(&home, &session)["my_name"],
        json!(" 米羽 "),
        "原样记"
    );
    let reply = client
        .call("s2", "session.send", said(&session, "8802", Value::Null))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert!(
        last_venue(&home, &session).get("my_name").is_none(),
        "没写的不写"
    );
}

#[tokio::test]
async fn a_bad_my_name_records_nothing() {
    let home = Home::new();
    let (mut client, session) = chat(&home).await;
    let before = home.log(&session).len();
    for (id, bad) in [
        ("b1", json!("   ")),
        ("b2", json!("米".repeat(65))),
        ("b3", json!("米\n羽")),
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
            said(&session, "8804", json!("米".repeat(64))),
        )
        .await;
    assert!(fits.get("error").is_none(), "64 个字正好：{fits}");
    let without_as = json!({"session": session, "text": "在吗",
        "venue": {"msg": "8805", "my_name": "米羽", "ambient": true}});
    let reply = client.call("n1", "session.send", without_as).await;
    assert!(reply.get("error").is_some(), "不带 as 的不收：{reply}");
    assert_eq!(home.log(&session).len(), before + 1, "写错的什么都没记");
}
