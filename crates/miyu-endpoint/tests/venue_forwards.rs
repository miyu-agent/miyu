//! 场所消息里的合并转发（施工 O-34，`docs/construction/O-34-合并转发.md`，核心审形状时定的五条）：`venue.forwards` 原样记进
//! `message.user`，只有转发、正文空的照旧 `empty_message`；写法不对的、单层超了的（套 4 层、一个转发 101 条、一条正文超过
//! 4 KiB、名字超过 64 个字符或带换行、空列表、`more` 写 0、空的 `nodes`、多写格）`bad_params`；整条消息合起来超了的（101 条、名字和正文 16 KiB 以上）
//! `bad_params`，`data.limit` 说是哪一样；都什么都不记。

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

/// 代表主人旁听地说一句，`text` 是正文，场所的格另带 `forwards`。
fn said(session: &str, text: &str, forwards: Value) -> Value {
    json!({"session": session, "text": text, "as": {"external": "qq:10001"},
        "venue": {"msg": "8810", "ambient": true, "forwards": forwards}})
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
async fn forwards_are_kept_as_written_and_a_message_with_only_one_is_empty() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let forwards = json!([
        {"nodes": [
            {"name": "阿杰", "text": "[forward]", "forwards": nested(2)},
            {"name": "老王", "text": "看\n下面", "media": [{"kind": "image", "id": "i-1", "size": 2048}]},
        ], "more": 12},
        {"nodes": [node("小林", "")]},
    ]);
    let reply = client
        .call(
            "s1",
            "session.send",
            said(&session, "[forward]", forwards.clone()),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let log = home.log(&session);
    let Body::MessageUser(message) = &log.last().expect("记下了").body else {
        panic!("应该是 message.user");
    };
    let written = serde_json::to_value(message.venue.as_ref().expect("有场所的格")).unwrap();
    assert_eq!(written["forwards"], forwards, "原样记下");
    // 转发不算内容（核心审时定）：正文里没有占位、也没带东西的照旧 `empty_message`，什么都不记。
    let before = home.log(&session).len();
    let reply = client
        .call("s2", "session.send", said(&session, "", forwards))
        .await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
    assert_eq!(home.log(&session).len(), before);
}

#[tokio::test]
async fn bad_forwards_record_nothing() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let before = home.log(&session).len();
    let one = |nodes: Value| json!([{"nodes": nodes}]);
    let many = |count: usize| Value::Array((0..count).map(|_| node("阿杰", "嗯")).collect());
    for (n, forwards) in [
        json!([]),
        nested(4),
        one(json!([])),
        one(many(101)),
        json!([{"nodes": [node("阿杰", "嗯")], "more": 0}]),
        json!([{"nodes": [node("阿杰", "嗯")], "more": -1}]),
        one(json!([node("阿杰", &"字".repeat(1366))])),
        one(json!([node(&"名".repeat(65), "嗯")])),
        one(json!([node("阿杰\n老板", "嗯")])),
        one(json!([node("  ", "嗯")])),
        one(json!([{"name": "阿杰"}])),
        one(json!([{"name": "阿杰", "text": "嗯", "time": 1}])),
        one(json!([{"name": "阿杰", "text": "嗯", "forwards": []}])),
        one(json!([{"name": "阿杰", "text": "嗯", "media": [{"kind": "gif", "id": "x"}]}])),
        json!([{"nodes": [node("阿杰", "嗯")], "id": "f-1"}]),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(
                &format!("b{n}"),
                "session.send",
                said(&session, "看", forwards),
            )
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
        assert!(
            reply["error"]["data"].get("limit").is_none(),
            "{n}：{reply}"
        );
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
}

/// 整条消息合起来的上限：条数、名字和正文的字节数，`data.limit` 说是哪一样；正好在上限上的收。
#[tokio::test]
async fn the_whole_message_has_its_own_limits() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let block = |count: usize, text: &str| json!({"nodes": (0..count).map(|_| node("阿杰", text)).collect::<Vec<_>>()});
    // 两个转发各 50 条、共 100 条：收。再多一条：超了。
    let fits = json!([block(50, "嗯"), block(50, "嗯")]);
    let reply = client
        .call("w1", "session.send", said(&session, "看", fits))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let before = home.log(&session).len();
    // 名字「阿杰」6 字节：4 条各 4090 字节的正文合起来 16384 字节正好，多一个字节就超。
    let exactly = "a".repeat(4096 - 6);
    for (n, forwards, limit) in [
        (
            2,
            json!([block(50, "嗯"), block(51, "嗯")]),
            "forward_nodes",
        ),
        (
            3,
            json!([{"nodes": [{"name": "阿杰", "text": "[forward]", "forwards": [block(100, "嗯")]}]}]),
            "forward_nodes",
        ),
        (
            4,
            json!([block(4, &exactly), {"nodes": [node("x", "")]}]),
            "forward_bytes",
        ),
    ] {
        let reply = client
            .call(
                &format!("w{n}"),
                "session.send",
                said(&session, "看", forwards),
            )
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
        assert_eq!(reply["error"]["data"]["limit"], limit, "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    let reply = client
        .call(
            "w5",
            "session.send",
            said(&session, "看", json!([block(4, &exactly)])),
        )
        .await;
    assert!(reply.get("error").is_none(), "正好 16 KiB 的收：{reply}");
}
