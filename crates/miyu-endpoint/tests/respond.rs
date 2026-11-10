//! `session.respond`（施工 O-14 上，`docs/construction/O-14-照记下的开一轮（上）.md`）：照旁听记下的几条开一轮，`triggers` 排好
//! 去重，事实接在内核的后面、`by` 是这个连接（本机的是主人）；写错的什么都不记；不是旁听的、当过触发的拒，`data.messages`
//! 是那几条；同一个请求编号再发只算一次。不写 `to` 的照事实开（施工 O-64）。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 照对应表起来的核心，连上、握手，找到主人（`qq:10001`）的私聊，旁听记下两条，交回连接、会话编号和两条的序号。
async fn overheard_twice(home: &Home, script: &Script) -> (Client, String, Vec<u64>) {
    let mut client = Client::connect(bound_core(home, script, Catalog::default()));
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
        .expect("有编号")
        .to_string();
    let mut seqs = Vec::new();
    for (n, msg) in ["8801", "8802"].into_iter().enumerate() {
        let reply = client
            .call(
                &format!("o{n}"),
                "session.send",
                json!({"session": session, "text": format!("第 {n} 句"), "as": {"external": "qq:10001"},
                       "venue": {"msg": msg, "ambient": true}}),
            )
            .await;
        seqs.push(
            reply["result"]["events"][0]
                .as_u64()
                .unwrap_or_else(|| panic!("{reply}")),
        );
    }
    (client, session, seqs)
}

fn respond(session: &str, to: Value) -> Value {
    json!({"session": session, "to": to, "facts": [{"kind": "judge", "text": "<judge/>\n"}]})
}

#[tokio::test]
async fn respond_opens_a_turn_on_what_was_overheard() {
    let home = Home::new();
    let (mut client, session, seqs) =
        overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let (a, b) = (seqs[0], seqs[1]);
    let reply = client
        .call("r1", "session.respond", respond(&session, json!([b, a, b])))
        .await;
    let events: Vec<u64> = reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let started = log
        .iter()
        .find(|event| event.seq.get() == events[0])
        .expect("记下了");
    let Body::TurnStarted(turn) = &started.body else {
        panic!("第一条是开这一轮：{started:?}");
    };
    let triggers: Vec<u64> = turn.triggers.iter().map(|seq| seq.get()).collect();
    assert_eq!(triggers, [a, b], "排好、去重");
    assert_eq!(turn.trigger.map(|seq| seq.get()), Some(b));
    let fact = log
        .iter()
        .find(|event| event.seq.get() == *events.last().unwrap())
        .expect("记下了");
    assert!(
        matches!(&fact.body, Body::ContextInjected(injected) if injected.text == "<judge/>\n"),
        "{fact:?}"
    );
    assert_eq!(fact.by, owner(), "本机的连接记成主人");
    // 同一个请求编号原样再发只算一次，回应一样；换了参数的拒（施工 YC-7）。
    let again = client
        .call("r1", "session.respond", respond(&session, json!([b, a, b])))
        .await;
    assert_eq!(again["result"], reply["result"]);
    let changed = client
        .call("r1", "session.respond", respond(&session, json!([b, a])))
        .await;
    assert_eq!(reason(&changed), Some("id_reused"), "{changed}");
}

/// 不写 `to`（施工 O-64，通讯平台到点看一眼私聊）：事实先记下，由它开一轮。
#[tokio::test]
async fn respond_without_messages_opens_a_turn_on_the_facts() {
    let home = Home::new();
    let (mut client, session, _) = overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let reply = client
        .call(
            "g1",
            "session.respond",
            json!({"session": session, "facts": [{"kind": "glance", "text": "<glance/>"}]}),
        )
        .await;
    let events: Vec<u64> = reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let at = |seq: u64| {
        log.iter()
            .find(|event| event.seq.get() == seq)
            .expect("记下了")
    };
    assert!(
        matches!(&at(events[0]).body, Body::ContextInjected(injected) if injected.text == "<glance/>"),
        "{:?}",
        at(events[0])
    );
    let Body::TurnStarted(turn) = &at(events[1]).body else {
        panic!("接着开这一轮：{:?}", at(events[1]));
    };
    assert_eq!(turn.trigger.map(|seq| seq.get()), Some(events[0]));
    assert!(turn.triggers.is_empty());
}

#[tokio::test]
async fn bad_requests_record_nothing_and_refusals_name_the_messages() {
    let home = Home::new();
    let (mut client, session, seqs) =
        overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let before = home.log(&session).len();
    let many: Vec<u64> = (1..=65).collect();
    for (n, params) in [
        json!({"session": session, "to": []}),
        json!({"session": session}),
        json!({"session": session, "facts": []}),
        json!({"session": session, "to": many}),
        json!({"session": session, "to": [0]}),
        json!({"session": session, "to": [seqs[0]], "facts": [{"kind": "Not A Kind", "text": "x"}]}),
        json!({"session": session, "to": [seqs[0]], "facts": [{"kind": "judge", "text": "x".repeat(4097)}]}),
        json!({"session": session, "to": [seqs[0]], "colour": "red"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "session.respond", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    let reply = client
        .call(
            "n1",
            "session.respond",
            respond(&session, json!([seqs[0], 1])),
        )
        .await;
    assert_eq!(reason(&reply), Some("not_ambient"), "{reply}");
    assert_eq!(reply["error"]["data"]["messages"], json!([1]));
    client
        .call("r1", "session.respond", respond(&session, json!([seqs[0]])))
        .await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "n2",
            "session.respond",
            respond(&session, json!([seqs[1], seqs[0]])),
        )
        .await;
    assert_eq!(reason(&reply), Some("already_answered"), "{reply}");
    assert_eq!(reply["error"]["data"]["messages"], json!([seqs[0]]));
}

fn owner() -> miyu_kernel::origin::By {
    miyu_kernel::origin::By::Person(miyu_kernel::origin::Person::default())
}

/// 正在跑一轮时（施工 O-14 下）：并进这一轮，`turn.joined` 带回合编号；打断以后不另开一轮。
#[tokio::test]
async fn respond_while_running_joins_the_turn() {
    let home = Home::new();
    let (mut client, session, seqs) = overheard_twice(&home, &Script::new([Play::Holds])).await;
    let opened = client
        .call(
            "s1",
            "session.send",
            json!({"session": session, "text": "@她 在吗", "as": {"external": "qq:10001"}}),
        )
        .await;
    assert!(opened.get("error").is_none(), "{opened}");
    let reply = client
        .call("r1", "session.respond", respond(&session, json!([seqs[1]])))
        .await;
    let events: Vec<u64> = reply["result"]["events"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    let log = home.log(&session);
    let joined = log
        .iter()
        .find(|event| event.seq.get() == *events.last().unwrap())
        .expect("记下了");
    let Body::TurnJoined(turn_joined) = &joined.body else {
        panic!("最后一条是并进去：{joined:?}");
    };
    assert_eq!(
        turn_joined
            .triggers
            .iter()
            .map(|seq| seq.get())
            .collect::<Vec<_>>(),
        [seqs[1]]
    );
    assert!(joined.turn.is_some(), "带回合编号");
    let stopped = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "send"}),
        )
        .await;
    assert!(stopped.get("error").is_none(), "{stopped}");
    home.until_turns(&session, 1).await;
    let started = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count();
    assert_eq!(started, 1, "打断以后不由它另开一轮");
}

/// 带了 `tail` 的（施工 RT-1）：记在 `turn.started` 上，发给模型的请求带着它；空的当没写。
#[tokio::test]
async fn a_tail_is_recorded_on_the_turn_and_sent_with_the_request() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。")]);
    let (mut client, session, seqs) = overheard_twice(&home, &script).await;
    let mut params = respond(&session, json!([seqs[0]]));
    params["tail"] = json!("（群里的提示）");
    let reply = client.call("t1", "session.respond", params).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    let mut params = respond(&session, json!([seqs[1]]));
    params["tail"] = json!("");
    let reply = client.call("t2", "session.respond", params).await;
    assert!(reply.get("error").is_none(), "空的当没写：{reply}");
    home.until_turns(&session, 2).await;
    let tails: Vec<Option<String>> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(turn) => Some(turn.tail.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(tails, [Some("（群里的提示）".to_string()), None]);
    let sent: Vec<Option<String>> = script
        .requests()
        .into_iter()
        .map(|(_, request)| request.tail)
        .collect();
    assert_eq!(sent, [Some("（群里的提示）".to_string()), None]);
}

/// `tail` 不是字符串的、超过 2 KiB 的：`bad_params`，`data.field` 是 `tail`，什么都不记（施工 RT-1）。
#[tokio::test]
async fn a_bad_tail_names_the_field_and_records_nothing() {
    let home = Home::new();
    let (mut client, session, seqs) =
        overheard_twice(&home, &Script::new([Play::Says("在。")])).await;
    let before = home.log(&session).len();
    for (n, tail) in [json!(7), json!(["x"]), json!("x".repeat(2049))]
        .into_iter()
        .enumerate()
    {
        let mut params = respond(&session, json!([seqs[0]]));
        params["tail"] = tail;
        let reply = client
            .call(&format!("b{n}"), "session.respond", params)
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{n}：{reply}");
        assert_eq!(reply["error"]["data"]["field"], "tail", "{n}：{reply}");
    }
    assert_eq!(home.log(&session).len(), before, "什么都没记");
    let mut params = respond(&session, json!([seqs[0]]));
    params["tail"] = json!("x".repeat(2048));
    let reply = client.call("ok", "session.respond", params).await;
    assert!(reply.get("error").is_none(), "正好 2 KiB 的收：{reply}");
}
