//! 会话分叉 `session.fork`（施工 C-8 上，`docs/construction/C-8-会话分叉（上）.md`）：真核心走一遍。主线说完一轮分叉，支线
//! 和主线接着说同一句，两边的请求 system、工具面一样，消息前面那一段逐字节一样；主线在跑一轮时分叉，分叉点在这一轮之前；
//! 支线照继承来的旁听消息开一轮（不用再抄）；主线记过她发出去的话以后分叉，支线、主线回同一条，请求前面那一段一样；重启
//! 核心以后支线照样载入；再分叉、没有的会话拒。主线的话不在支线里渲染成 `[you]` 行由 `miyu-assemble` 的群聊近况测试守。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 分叉，交回子会话的编号和分叉点。
async fn fork(client: &mut Client, id: &str, session: &str) -> (String, u64) {
    let reply = client
        .call(id, "session.fork", json!({"session": session}))
        .await;
    let child = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string();
    let upto = reply["result"]["upto"].as_u64().expect("有分叉点");
    (child, upto)
}

/// 两次请求的消息，除了最后一条（这一轮新来的那一条）都逐字节一样；system、工具面一样。
fn same_prefix(a: &miyu_kernel::request::Request, b: &miyu_kernel::request::Request) {
    assert_eq!(a.system, b.system, "system 一样");
    assert_eq!(a.tools, b.tools, "工具面一样");
    assert_eq!(a.messages.len(), b.messages.len(), "{a:#?}\n---\n{b:#?}");
    let n = a.messages.len() - 1;
    assert_eq!(
        serde_json::to_string(&a.messages[..n]).expect("写得成"),
        serde_json::to_string(&b.messages[..n]).expect("写得成"),
        "前面那一段逐字节一样"
    );
}

#[tokio::test]
async fn a_fork_shares_the_prefix_and_loads_back_after_a_restart() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("支线。"),
        Play::Says("主线。"),
        Play::Says("又是支线。"),
    ]);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let last = home.log(&session).last().expect("有事件").seq.get();
    let (child, upto) = fork(&mut client, "f1", &session).await;
    assert_eq!(upto, last, "闲着的从最后一条分叉");
    let first = home.log(&child);
    assert_eq!(first[0].seq.get(), upto + 1, "接着分叉点往下数");
    let Body::SessionCreated(created) = &first[0].body else {
        panic!("第一条是 session.created");
    };
    let forked = created.forked.as_ref().expect("写着从哪分叉");
    assert_eq!(
        (forked.from.as_str(), forked.upto.get()),
        (session.as_str(), upto)
    );
    assert_eq!(
        created.parent.as_ref().map(|p| p.as_str()),
        Some(session.as_str())
    );

    client.say("s2", &child, "再来").await;
    home.until_turns(&child, 1).await;
    client.say("s3", &session, "再来").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    same_prefix(&requests[1].1, &requests[2].1);

    // 重启核心：支线照样载入，接着说照样接得上。
    drop(client);
    drop(core);
    let again = home.core(&script);
    let mut client = Client::connect(again);
    client.hello().await;
    client.say("s4", &child, "还在吗").await;
    home.until_turns(&child, 2).await;
    assert_eq!(script.requests().len(), 4, "支线载入以后又说了一轮");
}

/// 私聊场所。
fn private() -> Value {
    json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"})
}

/// 场所 `venue` 里旁听记下几条：交回连接、会话编号和几条的序号。
async fn overheard(
    home: &Home,
    script: &Script,
    venue: Value,
    texts: &[&str],
) -> (Client, String, Vec<u64>) {
    let mut client = Client::connect(bound_core(home, script, Catalog::default()));
    client.hello().await;
    let made = client.call("v1", "venue.session", venue).await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let mut seqs = Vec::new();
    for (n, text) in texts.iter().enumerate() {
        seqs.push(
            hear(
                &mut client,
                &session,
                &format!("o{n}"),
                &format!("88{n:02}"),
                text,
            )
            .await,
        );
    }
    (client, session, seqs)
}

/// 旁听记下一条，交回序号。
async fn hear(client: &mut Client, session: &str, id: &str, msg: &str, text: &str) -> u64 {
    let reply = client
        .call(
            id,
            "session.send",
            json!({"session": session, "text": text, "as": {"external": "qq:10001"},
                   "venue": {"msg": msg, "ambient": true}}),
        )
        .await;
    reply["result"]["events"][0]
        .as_u64()
        .unwrap_or_else(|| panic!("{reply}"))
}

fn respond(session: &str, to: Value) -> Value {
    json!({"session": session, "to": to,
           "facts": [{"kind": "judge", "text": "Another line is already replying to msg=8800.\n"}]})
}

#[tokio::test]
async fn a_fork_taken_during_a_turn_starts_before_it_and_answers_an_inherited_message() {
    let home = Home::new();
    let script = Script::new([Play::Holds, Play::Says("回 B。")]);
    let (mut client, session, seqs) =
        overheard(&home, &script, private(), &["A 的话", "B 的话"]).await;
    // 主线回 A，停在半路。
    client
        .call(
            "r1",
            "session.respond",
            json!({"session": session, "to": [seqs[0]]}),
        )
        .await;
    until("主线开了一轮", || {
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::TurnStarted(_)))
    })
    .await;
    let started = home
        .log(&session)
        .iter()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq.get())
        .expect("开了一轮");
    let (child, upto) = fork(&mut client, "f1", &session).await;
    assert_eq!(upto, started - 1, "分叉点在主线这一轮开始之前");
    // B 在分叉点以前旁听记下，支线照它在主线里的序号开一轮，不用再抄。
    let reply = client
        .call("r2", "session.respond", respond(&child, json!([seqs[1]])))
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&child, 1).await;
    let requests = script.requests();
    let branch = &requests.last().expect("有请求").1;
    let text = serde_json::to_string(&branch.messages).expect("写得成");
    assert!(text.contains("B 的话"), "{text}");
    assert!(
        text.contains("Another line is already replying"),
        "事实照桥带的记"
    );
    client
        .call("i1", "session.interrupt", json!({"session": session}))
        .await;
}

#[tokio::test]
async fn a_fork_and_the_main_line_answer_the_same_message_alike() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("在。"),
        Play::Says("支线。"),
        Play::Says("主线。"),
    ]);
    let (mut client, session, seqs) = overheard(&home, &script, private(), &["在吗"]).await;
    client
        .call(
            "r1",
            "session.respond",
            json!({"session": session, "to": [seqs[0]]}),
        )
        .await;
    home.until_turns(&session, 1).await;
    // 桥替主线记下她实际发出去的那句。
    let delivered = client
        .call(
            "a1",
            "events.append",
            json!({"session": session, "kind": "venue.delivered",
                   "body": {"line": session, "turn": started_turn(&home, &session), "to": ["qq:10001"], "msg": "9901", "text": "在。"}}),
        )
        .await;
    assert!(delivered["result"]["seq"].is_u64(), "{delivered}");
    let c = hear(&mut client, &session, "o9", "8899", "那你回我").await;
    let (child, _) = fork(&mut client, "f1", &session).await;
    client
        .call(
            "r2",
            "session.respond",
            json!({"session": child, "to": [c]}),
        )
        .await;
    home.until_turns(&child, 1).await;
    client
        .call(
            "r3",
            "session.respond",
            json!({"session": session, "to": [c]}),
        )
        .await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    same_prefix(&requests[1].1, &requests[2].1);
}

/// 主线最近开的那一轮的回合编号（`turn.started` 的序号）。
fn started_turn(home: &Home, session: &str) -> u64 {
    home.log(session)
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq.get())
        .expect("开过一轮")
}

#[tokio::test]
async fn only_a_main_session_that_exists_can_be_forked() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let (child, _) = fork(&mut client, "f1", &session).await;
    let again = client
        .call("f2", "session.fork", json!({"session": child}))
        .await;
    assert_eq!(
        reason(&again),
        Some("bad_params"),
        "分叉出来的不能再分叉：{again}"
    );
    let missing = client
        .call(
            "f3",
            "session.fork",
            json!({"session": "01a0f233-cfec-7023-8ed5-2a037a1d5ec9"}),
        )
        .await;
    assert_eq!(reason(&missing), Some("session_not_found"), "{missing}");
    let garbled = client
        .call("f4", "session.fork", json!({"session": "nope"}))
        .await;
    assert_eq!(reason(&garbled), Some("bad_params"));
}
