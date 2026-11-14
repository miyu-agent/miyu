//! 场所消息带被引用的那一条的内容（施工 QT-1，`docs/construction/QT-1-场所消息带引用的内容.md`）：`venue.quoted` 和转发里的一条
//! 同一个形状，原样记进 `message.user`；写法不对的、没写 `reply_to` 的、里面的转发套到第 4 层的 `bad_params`；条数、字节数算进
//! 整条消息合起来的上限，超了的照 `forwards` 的 `data.limit` 拒；都什么都不记。

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

/// 代表主人旁听地说一句，场所的格照 `venue` 再加上 `msg`、`ambient`。
fn said(session: &str, mut venue: Value) -> Value {
    venue["msg"] = json!("8810");
    venue["ambient"] = json!(true);
    json!({"session": session, "text": "对", "as": {"external": "qq:10001"}, "venue": venue})
}

/// 一条转发里的话。
fn node(name: &str, text: &str) -> Value {
    json!({"name": name, "text": text})
}

/// 套 `depth` 层的转发：每层一条，里面一层接在那一条的 `forwards` 里。
fn nested(depth: usize) -> Value {
    let mut forwards = json!([{"nodes": [node("小美", "最里面")]}]);
    for _ in 1..depth {
        forwards =
            json!([{"nodes": [{"name": "阿杰", "text": "[forward]", "forwards": forwards}]}]);
    }
    forwards
}

#[tokio::test]
async fn a_quote_is_kept_as_written() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let quoted = json!({"name": "阿杰", "text": "周六\n见",
        "media": [{"kind": "image", "id": "i-1", "size": 2048, "index": 1}], "forwards": nested(3)});
    let reply = client
        .call(
            "q1",
            "session.send",
            said(
                &session,
                json!({"reply_to": "7000", "quoted": quoted.clone()}),
            ),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let log = home.log(&session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    let written = serde_json::to_value(message.venue.as_ref().expect("有场所的格")).unwrap();
    assert_eq!(written["quoted"], quoted, "原样记下");
}

#[tokio::test]
async fn bad_quotes_record_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    let replying = |quoted: Value| json!({"reply_to": "7000", "quoted": quoted});
    for (n, venue) in [
        json!({"quoted": node("阿杰", "嗯")}),
        replying(json!({"name": "阿杰", "text": "[forward]", "forwards": nested(4)})),
        replying(node("阿杰", &"字".repeat(1366))),
        replying(node("  ", "嗯")),
        replying(json!({"name": "阿杰"})),
        replying(json!({"name": "阿杰", "text": "嗯", "forwards": []})),
        replying(json!({"name": "阿杰", "text": "嗯", "media": [{"kind": "image", "id": "i", "shown": true}]})),
        replying(json!({"name": "阿杰", "text": "嗯", "time": 1})),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("b{n}"), "session.send", said(&session, venue))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
        assert!(
            reply["error"]["data"].get("limit").is_none(),
            "{n}：{reply}"
        );
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}

/// 引用的那一条和它里面的转发算进整条消息的上限：100 条、名字和正文 16 KiB，超了照 `forwards` 的 `data.limit` 拒。
#[tokio::test]
async fn a_quote_counts_toward_the_whole_message_limits() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let block = |count: usize, text: &str| json!({"nodes": (0..count).map(|_| node("阿杰", text)).collect::<Vec<_>>()});
    // 转发 99 条，加上引用的那一条：100 条，收。
    let fits =
        json!({"reply_to": "7000", "quoted": node("阿杰", "嗯"), "forwards": [block(99, "嗯")]});
    let reply = client
        .call("w1", "session.send", said(&session, fits))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let before = home.log(&session).len();
    // 名字「阿杰」6 字节：4 条各 4090 字节的正文合起来 16384 字节正好，引用的那一条再多一个字节就超。
    let exactly = "a".repeat(4096 - 6);
    for (n, venue, limit) in [
        (
            2,
            json!({"reply_to": "7000", "quoted": node("阿杰", "嗯"), "forwards": [block(100, "嗯")]}),
            "forward_nodes",
        ),
        (
            3,
            json!({"reply_to": "7000", "quoted": {"name": "阿杰", "text": "", "forwards": [block(100, "嗯")]}}),
            "forward_nodes",
        ),
        (
            4,
            json!({"reply_to": "7000", "quoted": node("x", ""), "forwards": [block(4, &exactly)]}),
            "forward_bytes",
        ),
    ] {
        let reply = client
            .call(&format!("w{n}"), "session.send", said(&session, venue))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
        assert_eq!(reply["error"]["data"]["limit"], limit, "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}
