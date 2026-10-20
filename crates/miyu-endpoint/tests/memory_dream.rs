//! 现在就整理记忆（施工 R-7 补，`docs/blueprint/memory.md` 第七条第 9 款，`protocol.md` 的 `memory.dream`、`/dream`）：真核心，
//! 请求模型是真的路由、整理记忆照 `models.chat` 发给假服务器。
//!
//! `memory.dream` 照人格那一间现在就合、交回几样数，没有要整理的是零、不发；在后台答，同一个连接后面的请求先回；同一间同时
//! 两个的回 `memory_busy`；没装人格记忆的回 `memory_not_installed`。`/dream` 在命令表里，在会话里先抽这一段再合，回执报数字；
//! 在后台答也先见结果、后见回应：订阅着这个会话的，它那条 `command.ran` 先到。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Body;

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 一家 `a` 在假服务器上，`models.chat` 是 `a/m`（整理记忆没写的照它）。
fn config(server: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
        server.base_url
    )
}

async fn connected(home: &Home, server: &Server) -> Client {
    home.write("system/config.toml", &config(server));
    let mut client = Client::connect(routed(home, &[], data(profiles(json!({})))));
    client.hello().await;
    client
}

/// 合并交回的一段，晚 `wait` 才回。
fn slow(text: &str, wait: Duration) -> Reply {
    let mut reply = said(text);
    reply.body.insert(0, Piece::Wait(wait));
    reply
}

async fn remember(client: &mut Client, id: &str, text: &str) {
    let reply = client
        .call(
            id,
            "memory.remember",
            json!({"persona": "engineer", "class": "user", "text": text}),
        )
        .await;
    assert!(reply["result"]["id"].is_string(), "{reply}");
}

#[tokio::test]
async fn a_dream_merges_now_and_says_how_much() {
    let server = Server::start(vec![
        said(
            &json!({"retired": [{"id": "m1", "kind": "repeat", "by": "m2", "why": "重复"}], "summary": "用户养猫。"}).to_string(),
        ),
        said(&json!({"keywords": []}).to_string()),
        said("{}"),
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    remember(&mut client, "c2", "用户养了一只猫，叫团子").await;
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"given": 2, "revised": 0, "retired": 1, "summary": true}),
        "{reply}"
    );
    let again = client
        .call("d2", "memory.dream", json!({"persona": "engineer"}))
        .await;
    // 没有新记的照样重看轮到的旧记忆（施工 R-19）：留下的那一条。
    assert_eq!(
        again["result"],
        json!({"given": 1, "revised": 0, "retired": 0, "summary": false}),
        "{again}"
    );
    assert_eq!(
        server.received().len(),
        3,
        "合并、接着补关键词（施工 R-15 下）、重看各一次；那一条补过关键词了，不再补"
    );
}

#[tokio::test]
async fn a_dream_is_answered_in_the_background_and_one_at_a_time() {
    // 整理那一次的模型回答停在闸前，等后面两个请求都回了才放行：后面的先回不靠时序。原来让它等 800 毫秒，Windows 的 CI 上
    // 列一遍比它慢，偶发红（K-3 修的 CI 上碰到）。整理要是挡住了后面的，两个都等不到，六十秒报错。
    let gate = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut held = said("{}");
    held.body
        .insert(0, Piece::Gate(std::sync::Arc::clone(&gate)));
    let server = Server::start(vec![held, said("{}")]).await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    let dream = |id: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "memory.dream", "params": {"persona": "engineer"}})
            .to_string()
    };
    client.line(&dream("d1")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    client.line(&dream("d2")).await;
    client
        .line(
            &json!({"jsonrpc": "2.0", "id": "l1", "method": "memory.list", "params": {"persona": "engineer"}})
                .to_string(),
        )
        .await;
    let mut order = Vec::new();
    let mut replies = Vec::new();
    while order.len() < 3 {
        let Some(message) = client.next_within(Duration::from_secs(20)).await else {
            // 整理挡住了后面的：放开闸让它做完再报，不然收尾时还在等它。
            let _released = tokio::time::timeout(Duration::from_secs(5), gate.wait()).await;
            panic!("整理挡住了后面的请求：{order:?}");
        };
        if let Some(id) = message["id"].as_str() {
            order.push(id.to_string());
            replies.push(message.clone());
            if order.len() == 2 {
                gate.wait().await;
            }
        }
    }
    assert_eq!(
        order.last().map(String::as_str),
        Some("d1"),
        "后面的请求先回：{order:?}"
    );
    let busy = replies
        .iter()
        .find(|reply| reply["id"] == "d2")
        .expect("d2 回了");
    assert_eq!(reason(busy), Some("memory_busy"), "{busy}");
}

#[tokio::test]
async fn without_the_package_there_is_no_dream() {
    let server = Server::start(vec![said("{}")]).await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    let removed = client
        .call("r1", "package.remove", json!({"package": "memory"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
    assert!(server.received().is_empty());
}

/// 抽取交回的候选：1 到 40 每一轮各一条 `text`（只有这一段里真有的那几轮留得下）。
fn every_turn(text: &str) -> Reply {
    let memories: Vec<Value> = (1..=40)
        .map(|turn| json!({"class": "user", "text": text, "turn": turn}))
        .collect();
    said(&json!({ "memories": memories }).to_string())
}

#[tokio::test]
async fn slash_dream_extracts_the_session_merges_in_the_background_and_says_how_much() {
    let server = Server::start(vec![
        said("好。"),
        said("养猫"),
        every_turn("用户养了一只猫"),
        // 抽取记下以后、合并落完以后各写一次关键词（施工 R-15 下）。
        said(&json!({"keywords": []}).to_string()),
        slow(
            &json!({"summary": "用户养猫。"}).to_string(),
            Duration::from_millis(500),
        ),
        said(&json!({"keywords": []}).to_string()),
        // 第二次 `/dream`：没有新记的，重看那一条（施工 R-19）。
        said("{}"),
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    let session = client
        .create_with(
            "c1",
            json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
        )
        .await;
    client.say("s1", &session, "我养了一只猫").await;
    // 答完头一轮起标题，也照 `models.chat` 发：起好了再叫，请求的先后才定。
    until("起好标题", || {
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::MetaChanged(_)))
    })
    .await;
    let subscribed = client.subscribe("w1", &session).await;
    assert!(subscribed.get("result").is_some(), "{subscribed}");
    let line = |id: &str, method: &str, params: Value| {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    };
    client
        .line(&line(
            "k1",
            "command.run",
            json!({"session": session, "text": "/dream"}),
        ))
        .await;
    client
        .line(&line("k2", "command.catalog", json!({"session": session})))
        .await;
    let mut replies = Vec::new();
    let mut ran_before_reply = false;
    while replies.len() < 2 {
        let message = client.next().await.expect("连着");
        if message["id"].is_string() {
            replies.push(message);
        } else if message["params"]["event"]["kind"] == "command.ran" {
            ran_before_reply = replies.iter().all(|reply| reply["id"] != "k1");
        }
    }
    assert!(ran_before_reply, "先见结果、后见回应：command.ran 先到");
    let [catalog, dreamed] = replies.as_slice() else {
        unreachable!("两条")
    };
    assert_eq!(catalog["id"], "k2", "后面的请求先回");
    assert!(
        catalog["result"]["commands"]
            .as_array()
            .expect("有")
            .iter()
            .any(|command| command["name"] == "dream"),
        "{catalog}"
    );
    assert_eq!(dreamed["result"]["command"], "dream", "{dreamed}");
    // 是 0 的不写、末尾写用时（施工 R-22 再补），用时每次不同，只看头尾。
    let said = dreamed["result"]["said"].as_str().unwrap_or_default();
    assert!(
        said.starts_with("整理完成：检查 1 条，没有要改的，摘要已更新，用时 ")
            && said.ends_with("s。"),
        "{dreamed}"
    );
    assert_eq!(
        server.received().len(),
        6,
        "聊了一轮、起了标题、抽了一次、合了一次，抽完、合完各写一次关键词（施工 R-15 下）"
    );
    assert!(
        home.log(&session)
            .iter()
            .any(|event| matches!(&event.body, Body::CommandRan(ran) if ran.command == "dream"))
    );
    client
        .line(&line(
            "k3",
            "command.run",
            json!({"session": session, "text": "/dream"}),
        ))
        .await;
    let (pushed, again) = client.until_reply("k3").await;
    let said = again["result"]["said"].as_str().unwrap_or_default();
    assert!(
        said.starts_with("整理完成：检查 1 条，没有要改的，用时 ") && said.ends_with("s。"),
        "没有新记的照样重看（施工 R-19）：{again}"
    );
    assert!(
        pushed
            .iter()
            .any(|push| push["params"]["event"]["cause"] == "k3/ran"),
        "{pushed:?}"
    );
}

/// 订阅着这一间的边整理边收到她写完的（施工 R-21）：`memory.dreaming` 照写完的先后，带这一批的 `upto` 和订阅时写的人格；
/// 开始合的那一行带这一批交的是哪几条。
#[tokio::test]
async fn subscribers_see_what_she_writes_while_dreaming() {
    let server = Server::start(vec![
        said(
            &json!({"revised": [{"id": "m2", "text": "用户养了一只橘猫，叫团子"}], "retired": [{"id": "m1", "kind": "repeat", "by": "m2", "why": "重复"}], "summary": "用户养猫。"}).to_string(),
        ),
        said(&json!({"keywords": []}).to_string()),
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    remember(&mut client, "c2", "用户养了一只猫，叫团子").await;
    let subscribe = json!({"jsonrpc": "2.0", "id": "s1", "method": "subscribe", "params": {"stream": "memory", "persona": "engineer"}});
    client.line(&subscribe.to_string()).await;
    client.until_reply("s1").await;
    let dream = json!({"jsonrpc": "2.0", "id": "d1", "method": "memory.dream", "params": {"persona": "engineer"}});
    client.line(&dream.to_string()).await;
    let (pushed, reply) = client.until_reply("d1").await;
    assert_eq!(reply["result"]["retired"], 1, "{reply}");
    let dreaming: Vec<&Value> = pushed
        .iter()
        .filter(|push| push["method"] == "memory.dreaming")
        .map(|push| &push["params"])
        .collect();
    assert_eq!(
        dreaming,
        [
            &json!({"upto": 2, "kind": "revised", "id": "m2", "text": "用户养了一只橘猫，叫团子", "persona": "engineer"}),
            &json!({"upto": 2, "kind": "retired", "id": "m1", "retire": "repeat", "kept": "m2", "why": "重复", "persona": "engineer"}),
            &json!({"upto": 2, "kind": "summary", "chars": 5, "done": true, "persona": "engineer"}),
        ],
        "{pushed:?}"
    );
    let merging = pushed
        .iter()
        .find(|push| push["params"]["event"]["kind"] == "ext.memory.merging")
        .expect("推了开始合");
    assert_eq!(
        merging["params"]["event"]["body"]["given"],
        json!(["m1", "m2"])
    );
}

/// 剧本的一次性入口（施工 test-once）：测试核心照 `Script::once` 排的答整理，`memory.dream` 走得通、问的照先后记下；
/// 没排的剧本没有一次性入口，核心不交整理要的几样，回 `memory_unavailable`（头的端到端测试原来只测得到这一条）。
#[tokio::test]
async fn a_scripted_core_dreams_with_the_answers_it_was_given() {
    let merged = json!({"retired": [{"id": "m1", "kind": "repeat", "by": "m2", "why": "重复"}], "summary": "用户养猫。"});
    let script = miyu_session::testkit::Script::new([]).once([
        merged.to_string(),
        json!({"keywords": []}).to_string(),
        "{}".to_string(),
    ]);
    let home = Home::new();
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    remember(&mut client, "c2", "用户养了一只猫，叫团子").await;
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"given": 2, "revised": 0, "retired": 1, "summary": true}),
        "{reply}"
    );
    assert!(!script.asked().is_empty(), "整理经一次性入口问了");

    let plain = Home::new();
    let mut client = Client::connect(plain.core(&miyu_session::testkit::Script::new([])));
    client.hello().await;
    remember(&mut client, "c1", "用户养了一只猫").await;
    let reply = client
        .call("d1", "memory.dream", json!({"persona": "engineer"}))
        .await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
}

/// 叫停了的一趟：两条回应，和这期间的推送；`client`、`session` 接着用。
struct Hushed {
    _home: Home,
    _server: Server,
    client: Client,
    session: String,
    stop: Value,
    dreamed: Value,
    pushed: Vec<Value>,
}

fn line(id: &str, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

/// 会话里叫 `/dream`，合并的请求发出去以后打 `stop`（`/stop` 或 `/dream stop`）；订阅着记忆流。
async fn hushed_by(stop: &str) -> Hushed {
    let mut stalled = said("{}");
    stalled.body.insert(0, Piece::Stall);
    let server = Server::start(vec![
        said("好。"),
        said("养猫"),
        every_turn("用户养了一只猫"),
        said(&json!({"keywords": []}).to_string()),
        stalled,
    ])
    .await;
    let home = Home::new();
    let mut client = connected(&home, &server).await;
    let session = client
        .create_with(
            "c1",
            json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
        )
        .await;
    client.say("s1", &session, "我养了一只猫").await;
    until("起好标题", || {
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::MetaChanged(_)))
    })
    .await;
    let memory = json!({"stream": "memory", "persona": "engineer"});
    client.line(&line("m1", "subscribe", memory)).await;
    client.until_reply("m1").await;
    let dream = json!({"session": session, "text": "/dream"});
    client.line(&line("k1", "command.run", dream)).await;
    server.wait_stalled(1).await;
    let stopping = json!({"session": session, "text": stop});
    client.line(&line("k2", "command.run", stopping)).await;
    // 两条回应谁先到不一定：`/dream` 叫停了就回。
    let (mut pushed, mut replies) = (Vec::new(), Vec::new());
    while replies.len() < 2 {
        let message = client.next().await.expect("连着");
        match message["id"].as_str() {
            Some("k1" | "k2") => replies.push(message),
            _ => pushed.push(message),
        }
    }
    let reply = |id: &str| {
        replies
            .iter()
            .find(|reply| reply["id"] == id)
            .expect("回了")
            .clone()
    };
    let (stop, dreamed) = (reply("k2"), reply("k1"));
    let said = dreamed["result"]["said"].as_str().unwrap_or_default();
    assert!(
        said.starts_with("整理已中断，用时 ") && said.ends_with("s，下次再整理。"),
        "{dreamed}"
    );
    // 记忆流的推送另走转发任务，可能晚于回应到。
    while !pushed
        .iter()
        .any(|push: &Value| push.to_string().contains(r#""why":"stopped""#))
    {
        pushed.push(client.next().await.expect("记忆流推了放弃的那一批"));
    }
    Hushed {
        _home: home,
        _server: server,
        client,
        session,
        stop,
        dreamed,
        pushed,
    }
}

/// 会话的 `/stop` 连它叫的 `/dream` 一起停（施工 R-22）：正在合的那一批放弃，记忆流推一条 `merge_failed`（`stopped`），
/// `/dream` 照样回应，回执说中断了。
#[tokio::test]
async fn slash_stop_stops_the_sessions_dream() {
    let hushed = hushed_by("/stop").await;
    assert_eq!(hushed.stop["result"]["command"], "stop", "{}", hushed.stop);
    assert!(hushed.dreamed["result"]["said"].is_string());
    assert!(!hushed.pushed.is_empty());
}

/// `/dream stop` 只叫停整理（施工 R-22）：回执说停了；没在整理时回 `not_dreaming`，后面跟别的字的参数不对。
#[tokio::test]
async fn slash_dream_stop_stops_only_the_dream() {
    let mut hushed = hushed_by("/dream stop").await;
    assert_eq!(
        hushed.stop["result"]["said"], "已停止整理。",
        "{}",
        hushed.stop
    );
    let again = json!({"session": hushed.session, "text": "/dream stop"});
    hushed.client.line(&line("k3", "command.run", again)).await;
    let (_, again) = hushed.client.until_reply("k3").await;
    assert_eq!(reason(&again), Some("not_dreaming"), "{again}");
    let wrong = json!({"session": hushed.session, "text": "/dream later"});
    hushed.client.line(&line("k4", "command.run", wrong)).await;
    let (_, wrong) = hushed.client.until_reply("k4").await;
    assert_eq!(reason(&wrong), Some("bad_params"), "{wrong}");
}
