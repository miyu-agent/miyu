//! 会话中途换人格（施工 P-13，`docs/designs/16-人格与预设.md` Y5）：真核心走一遍。`session.configure` 写 `persona`，内核记一条人
//! 换的 `session.policy_changed`，马上压缩一轮（摘要请求还是旧人格的 system）；下一个回合换上新人格，从摘要接着聊。跟着人格的
//! 记忆从这一轮起在新人格那一间：以前记的留在旧的那一间，新记的进新的。会话列表、订阅回应照换成的写。没有的人格拒、什么都
//! 不记；和现在一样的什么都不做。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 记忆列出来的字。
fn texts(reply: &Value) -> Vec<String> {
    reply["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("有 memories：{reply}"))
        .iter()
        .filter_map(|memory| memory["text"].as_str().map(str::to_string))
        .collect()
}

/// 人在会话里记一条。
async fn remember(client: &mut Client, id: &str, session: &str, text: &str) {
    let reply = client
        .call(
            id,
            "command.run",
            json!({"session": session, "text": format!("/remember {text}")}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
}

/// 照 `params` 列记忆。
async fn listed(client: &mut Client, id: &str, params: Value) -> Vec<String> {
    texts(&client.call(id, "memory.list", params).await)
}

/// 会话列表里编号是 `session` 的那一项。
async fn item(client: &mut Client, id: &str, session: &str) -> Value {
    let sessions = client.call(id, "session.list", json!({})).await;
    sessions["result"]["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["session"] == session))
        .cloned()
        .unwrap_or_else(|| panic!("{sessions}"))
}

/// 照人格 `persona` 那一间搜以前的对话，搜到为止（最多十秒：回合库在日志落盘以后写）。
async fn turns(client: &mut Client, query: &str, persona: &str) -> Vec<Value> {
    for n in 0..500 {
        let reply = client
            .call(
                &format!("t{n}"),
                "memory.turns",
                json!({"query": query, "persona": persona}),
            )
            .await;
        let found = reply["result"]["turns"]
            .as_array()
            .unwrap_or_else(|| panic!("有 turns：{reply}"))
            .clone();
        if !found.is_empty() {
            return found;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("十秒内 {persona} 那一间搜不到 {query}")
}

/// 换人格。
async fn recast(client: &mut Client, id: &str, session: &str, persona: Value) -> Value {
    client
        .call(
            id,
            "session.configure",
            json!({"session": session, "persona": persona}),
        )
        .await
}

#[tokio::test]
async fn a_session_switches_persona_after_a_compaction() {
    let home = Home::new();
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu.\n",
    );
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client
        .create_with("c", json!({"cwd": "~", "persona": "engineer"}))
        .await;
    remember(&mut client, "k1", &session, "用户养猫").await;
    let first = format!("我的显卡是 N 卡{}", "x".repeat(70_000));
    client.say("s1", &session, &first).await;
    home.until_turns(&session, 1).await;

    let empty = recast(&mut client, "b", &session, json!("")).await;
    assert_eq!(reason(&empty), Some("bad_params"), "{empty}");
    let unknown = recast(&mut client, "u", &session, json!("nope")).await;
    assert_eq!(reason(&unknown), Some("unknown_persona"), "{unknown}");
    let same = recast(&mut client, "e", &session, json!("engineer")).await;
    assert_eq!(same["result"], json!({}), "{same}");
    let before = home.log(&session).len();
    assert!(
        !home.log(&session).iter().any(
            |event| matches!(&event.body, Body::PolicyChanged(changed) if changed.persona.is_some())
        ),
        "没有的、一样的什么都不记"
    );
    let reply = recast(&mut client, "r", &session, json!("miyu")).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    let changed = log[before..]
        .iter()
        .find(|event| {
            matches!(&event.body, Body::PolicyChanged(changed) if changed.persona.as_deref() == Some("miyu"))
        })
        .expect("记了要换成的");
    assert!(matches!(changed.by, By::Person(_)), "人换的");
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::ContextCompacted(_))),
        "马上压了一轮"
    );

    client.say("s2", &session, "接着聊天气").await;
    home.until_turns(&session, 3).await;
    let requests = script.requests();
    assert_eq!(requests.len(), 3);
    assert!(
        !requests[1].1.system.contains("You are Miyu."),
        "摘要照旧人格写：{}",
        requests[1].1.system
    );
    assert!(
        requests[2].1.system.starts_with("You are Miyu."),
        "下一个回合换上：{}",
        requests[2].1.system
    );
    assert!(
        serde_json::to_string(&requests[2].1.messages)
            .expect("写得成")
            .contains("The user said hi."),
        "从摘要接着聊"
    );

    remember(&mut client, "k2", &session, "用户喜欢喝茶").await;
    let here = listed(&mut client, "l1", json!({"session": session})).await;
    assert_eq!(here, ["用户喜欢喝茶"], "会话的记忆换到了新人格那一间");
    let old = listed(&mut client, "l2", json!({"persona": "engineer"})).await;
    assert_eq!(old, ["用户养猫"], "以前记的留在旧的那一间");
    // 回合库：换过来以后的那一轮进新的一间，以前的几轮只在旧的一间。
    turns(&mut client, "天气", "miyu").await;
    assert!(
        turns(&mut client, "显卡", "engineer").await.len() == 1,
        "以前的一轮在旧的一间"
    );
    let moved = client
        .call(
            "tm",
            "memory.turns",
            json!({"query": "显卡", "persona": "miyu"}),
        )
        .await;
    assert_eq!(
        moved["result"]["turns"],
        json!([]),
        "不搬进新的一间：{moved}"
    );

    assert_eq!(item(&mut client, "l3", &session).await["persona"], "miyu");
    // 另开一条连接订阅：推过来的事件不和这一条的回应搅在一起。
    let mut watching = Client::connect(Arc::clone(&core));
    watching.hello().await;
    let subscribed = watching
        .call(
            "w",
            "subscribe",
            json!({"stream": "events", "session": session}),
        )
        .await;
    assert_eq!(
        subscribed["result"]["persona"], "miyu",
        "订阅回应照换成的写：{subscribed}"
    );

    let forked = client
        .call("f", "session.fork", json!({"session": session}))
        .await;
    let child = forked["result"]["session"].as_str().expect("分叉出来了");
    assert_eq!(
        item(&mut client, "l6", child).await["persona"],
        "miyu",
        "分叉出来的照换成的"
    );

    let none = recast(&mut client, "n", &session, Value::Null).await;
    assert_eq!(none["result"], json!({}), "{none}");
    let shown = item(&mut client, "l4", &session).await;
    assert!(shown.get("persona").is_none(), "换成无人格的不写：{shown}");

    // 重启：换上的是 Miyu，记忆照旧在它那一间（换成无人格的还没生效）。
    core.stop_sessions().await;
    drop((client, watching));
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let here = listed(&mut client, "l5", json!({"session": session})).await;
    assert_eq!(here, ["用户喜欢喝茶"], "载入照换上的人格接记忆");
}
