//! 场所消息里的合并转发（施工 O-34，`internal/construction/O-34-合并转发.md`，核心审形状时定的五条）：`venue.forwards` 原样记进
//! `message.user`，只有转发、正文空的照旧 `empty_message`；写法不对的、单层不合的（套 9 层、名字超过 64 个字符或带换行、空列表、`more` 写 0、空的 `nodes`、多写格）
//! `bad_params`，什么都不记；整条消息合起来的条数、字节数不另设上限（施工 O-34 补）。

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
    for (n, forwards) in [
        json!([]),
        nested(9),
        one(json!([])),
        json!([{"nodes": [node("阿杰", "嗯")], "more": 0}]),
        json!([{"nodes": [node("阿杰", "嗯")], "more": -1}]),
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

/// 整条消息合起来不另设上限（施工 O-34 补，2026-11-19 项目主人定：合并转发不设影响体验的上限，这不是要熔断保护的地方）：
/// 原来所有转发合起来最多 100 条、名字和正文合起来 16 KiB、一条正文 4096 字节，超了整条拒收。现在都收，原样记下；剩下的
/// 天花板是连接上一行 1 MiB。套的层数留一个只防栈的上限 8：正常的转发碰不到。
#[tokio::test]
async fn forwards_have_no_caps_on_count_or_size() {
    let home = Home::new();
    let (mut client, session) = owners_chat(&home).await;
    let block = |count: usize, text: &str| json!({"nodes": (0..count).map(|_| node("阿杰", text)).collect::<Vec<_>>()});
    let long = "字".repeat(7000);
    for (n, forwards) in [
        // 一个转发 250 条：一个转发里几条也不设上限。
        json!([block(250, "嗯")]),
        // 三个转发各 100 条：合起来 300 条。
        json!([block(100, "嗯"), block(100, "嗯"), block(100, "嗯")]),
        // 套在里面的 100 条，连外面那一条 101 条。
        json!([{"nodes": [{"name": "阿杰", "text": "[forward]", "forwards": [block(100, "嗯")]}]}]),
        // 一条正文 21000 字节。
        json!([{"nodes": [node("阿杰", &long)]}]),
        // 名字和正文合起来 40 KiB 以上。
        json!([block(10, &"a".repeat(4096))]),
        // 套 8 层。
        nested(8),
    ]
    .into_iter()
    .enumerate()
    {
        let before = home.log(&session).len();
        let reply = client
            .call(
                &format!("w{n}"),
                "session.send",
                said(&session, "看", forwards.clone()),
            )
            .await;
        assert!(reply.get("error").is_none(), "{n}：{reply}");
        let log = home.log(&session);
        assert!(log.len() > before, "{n}：记下了");
        let kept = serde_json::to_value(log.last().expect("有一条")).expect("写得成 JSON");
        assert_eq!(kept["body"]["venue"]["forwards"], forwards, "{n}：原样记下");
    }
}
