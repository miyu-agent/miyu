//! 场所消息「戳一戳」（施工 VP-1，`internal/construction/VP-1-场所消息戳一戳.md`）：`venue.poke` 只收 `true`，带它的 `text` 可以是空的；
//! `poke_text` 可选，只跟着 `poke` 来，同名字的规矩（不是空白、最多 64 个字符、没有控制字符），原样记进 `message.user`。写错的
//! `bad_params`，不带 `poke` 的空消息照旧 `empty_message`；都什么都不记。

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

/// 代表主人旁听地发一条空的，场所的格照 `venue` 再加上 `msg`（im 写的是收到那一刻的毫秒数）、`ambient`。
fn poked(session: &str, mut venue: Value) -> Value {
    venue["msg"] = json!("1794905517642");
    venue["ambient"] = json!(true);
    json!({"session": session, "text": "", "as": {"external": "qq:10001"}, "venue": venue})
}

#[tokio::test]
async fn a_poke_without_words_is_kept_as_written() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    for (n, venue) in [
        json!({"poke": true, "name": "小林"}),
        json!({"poke": true, "name": "小林", "poke_text": "的小脑袋"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(
                &format!("p{n}"),
                "session.send",
                poked(&session, venue.clone()),
            )
            .await;
        assert!(reply.get("error").is_none(), "{n}：{reply}");
        let log = home.log(&session);
        let Body::MessageUser(message) = &log.last().expect("记下了").body else {
            panic!("应该是 message.user");
        };
        assert!(message.blocks.is_empty(), "{n}：没有内容块");
        let written = serde_json::to_value(message.venue.as_ref().expect("有场所的格")).unwrap();
        assert_eq!(written["poke"], true, "{n}");
        assert_eq!(
            written.get("poke_text"),
            venue.get("poke_text"),
            "{n}：原样记下"
        );
    }
}

#[tokio::test]
async fn bad_pokes_record_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    for (n, venue, why) in [
        (0, json!({"poke": false}), "bad_params"),
        (1, json!({"poke_text": "的小脑袋"}), "bad_params"),
        (2, json!({"poke": true, "poke_text": "  "}), "bad_params"),
        (
            3,
            json!({"poke": true, "poke_text": "字".repeat(65)}),
            "bad_params",
        ),
        (
            4,
            json!({"poke": true, "poke_text": "的\n小脑袋"}),
            "bad_params",
        ),
        (5, json!({"poke": "yes"}), "bad_params"),
        (6, json!({}), "empty_message"),
    ] {
        let reply = client
            .call(&format!("b{n}"), "session.send", poked(&session, venue))
            .await;
        assert_eq!(reason(&reply), Some(why), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}
