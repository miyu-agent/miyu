//! 会话钉自己的看图模型（施工 VM-1，`docs/blueprint/models.md`「怎么走」第十三条第 4 条）：`session.configure` 的 `vision`
//! 照这时的配置解析好记一条 `session.policy_changed`，下一轮替看图的请求发给它；`null` 去掉、回到跟着 `models.chat`
//! 那样跟着 `models.vision`；解析不出的 `unknown_model`、写法不对的 `bad_params`，什么都不记。`session.create` 也收：造好
//! 以后记一条，回应的 `events` 多它。`view.status` 钉了的带 `vision`，没钉的不写。
//!
//! 三台假服务器：`a` 是会话的模型 `a/m`（没写 `inputs`，看不了图），`b` 是配置的看图模型 `b/v`，`c` 是钉的 `c/w`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_http::testkit::{Reply, Server};

use crate::support::providers::{data, profiles, routed, said};
use crate::support::*;

/// 三家的配置：`models.vision` 是 `b/v`。
fn config(chat: &Server, configured: &Server, pinned: &Server) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nwindow = 100000\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n\
         [providers.c]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.c.models.w]\ninputs = [\"text\", \"image\"]\n\n\
         [models]\nchat = \"a/m\"\nvision = \"b/v\"\n",
        chat.base_url, configured.base_url, pinned.base_url
    )
}

/// `n` 份说「好。」的回答。
fn answers(n: usize) -> Vec<Reply> {
    (0..n).map(|_| said("好。")).collect()
}

/// 三台服务器，照 [`config`] 写好系统配置，请求模型是真路由的核心。
async fn three(home: &Home, pinned: usize) -> (Arc<Core>, [Server; 3]) {
    let servers = [
        Server::start(answers(8)).await,
        Server::start(answers(4)).await,
        Server::start(answers(pinned)).await,
    ];
    home.write(
        "etc/config.toml",
        &config(&servers[0], &servers[1], &servers[2]),
    );
    (routed(home, &[], data(profiles(json!({})))), servers)
}

/// 连上 `core`、握好手。
async fn client(core: &Arc<Core>) -> Client {
    let mut client = Client::connect(Arc::clone(core));
    client.hello().await;
    client
}

/// 一张 `side` × `side` 的 PNG 的开头：签名和 IHDR。边长不同，存进去是不同的 blob（同一张图一个会话只转述一次）。
fn png(side: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&side.to_be_bytes());
    bytes.extend_from_slice(&side.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 发一张边长 `side` 的图，等会话说完第 `turns` 轮。
async fn show(client: &mut Client, home: &Home, session: &str, side: u32, turns: usize) {
    let path = home.work.join(format!("{side}.png"));
    std::fs::write(&path, png(side)).expect("写得了");
    let put = client
        .call(&format!("p{side}"), "blob.put", json!({"path": path}))
        .await;
    let params = json!({"session": session, "text": "这是什么？", "attachments": [put["result"]]});
    let reply = client
        .call(&format!("s{side}"), "session.send", params)
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(session, turns).await;
}

/// 发一条 `session.configure {session, vision}`。
async fn pin(client: &mut Client, id: &str, session: &str, vision: Value) -> Value {
    let params = json!({"session": session, "vision": vision});
    client.call(id, "session.configure", params).await
}

/// 日志里换看图模型的那几条的 `body`，照先后。
fn pins(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| {
            event["kind"] == "session.policy_changed" && event["body"]["vision"].is_string()
        })
        .map(|event| event["body"].clone())
        .collect()
}

/// 一台服务器收到的请求里的 `model`，照先后。
fn models(server: &Server) -> Vec<Value> {
    server
        .received()
        .iter()
        .map(|received| {
            let body: Value = serde_json::from_slice(&received.body).expect("请求是 JSON");
            body["model"].clone()
        })
        .collect()
}

/// 新连上一个头订阅视图流：回应里整份会话状态。
async fn status(core: &Arc<Core>, session: &str) -> Value {
    let mut viewer = client(core).await;
    let reply = viewer
        .call(
            "v",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    reply["result"]["status"].clone()
}

#[tokio::test]
async fn a_pinned_vision_model_describes_and_null_goes_back_to_the_config() {
    let home = Home::new();
    let (core, [chat, configured, pinned]) = three(&home, 2).await;
    let mut client = client(&core).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    show(&mut client, &home, &session, 1, 1).await;
    assert_eq!(models(&configured), [json!("v")], "没钉的照 models.vision");
    assert!(pinned.received().is_empty());
    assert!(status(&core, &session).await.get("vision").is_none());
    let reply = pin(&mut client, "c2", &session, json!("c/w")).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(status(&core, &session).await["vision"], "c/w");
    let reply = pin(&mut client, "c3", &session, json!("c/w")).await;
    assert_eq!(reply["result"], json!({}), "一样的照收：{reply}");
    show(&mut client, &home, &session, 2, 2).await;
    assert_eq!(models(&pinned), [json!("w")], "钉了的下一轮发给它");
    assert_eq!(configured.received().len(), 1);
    let reply = pin(&mut client, "c4", &session, Value::Null).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(status(&core, &session).await.get("vision").is_none());
    show(&mut client, &home, &session, 3, 3).await;
    assert_eq!(models(&configured).len(), 2, "去掉了回到配置的");
    assert_eq!(pinned.received().len(), 1);
    assert_eq!(
        pins(&home, &session),
        [json!({"vision": "c/w"}), json!({"vision": ""})],
        "一样的不记，null 记成空的"
    );
    assert_eq!(chat.received().len(), 3, "主请求一轮一次");
}

#[tokio::test]
async fn what_model_refuses_vision_refuses_and_nothing_is_recorded() {
    let home = Home::new();
    let (core, _servers) = three(&home, 0).await;
    let mut client = client(&core).await;
    let session = client.create("c1", "~").await;
    for (n, vision) in ["d/x", "@nope", "nope"].into_iter().enumerate() {
        let reply = pin(&mut client, &format!("u{n}"), &session, json!(vision)).await;
        assert_eq!(reason(&reply), Some("unknown_model"), "{vision}：{reply}");
    }
    for (n, vision) in [json!(""), json!(3), json!(["c/w"])]
        .into_iter()
        .enumerate()
    {
        let reply = pin(&mut client, &format!("b{n}"), &session, vision.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{vision}：{reply}");
    }
    // 和别的一起写的：解析不出的什么都不换（模型那一格也不换）。
    let params = json!({"session": session, "model": "c/w", "vision": "d/x"});
    let reply = client.call("both", "session.configure", params).await;
    assert_eq!(reason(&reply), Some("unknown_model"), "{reply}");
    let changed = logged(&home, &session)
        .into_iter()
        .filter(|event| event["kind"] == "session.policy_changed")
        .count();
    assert_eq!(changed, 0, "什么都不记");
}

#[tokio::test]
async fn session_create_takes_vision_as_well() {
    let home = Home::new();
    let (core, [_chat, configured, pinned]) = three(&home, 1).await;
    let mut client = client(&core).await;
    let params = json!({"cwd": "~", "titles": false, "vision": "c/w"});
    let reply = client.call("c1", "session.create", params).await;
    assert_eq!(reply["result"]["events"], json!([1, 2]), "{reply}");
    let session = reply["result"]["session"].as_str().expect("造出了会话");
    assert_eq!(pins(&home, session), [json!({"vision": "c/w"})]);
    assert_eq!(status(&core, session).await["vision"], "c/w");
    show(&mut client, &home, session, 1, 1).await;
    assert_eq!(models(&pinned), [json!("w")], "第一轮就照钉的");
    assert!(configured.received().is_empty());
    let made = home.root.sessions().expect("读得了").len();
    let reply = client
        .call("c2", "session.create", json!({"vision": "@nope"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_model"), "{reply}");
    let reply = client
        .call("c3", "session.create", json!({"vision": 3}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(
        home.root.sessions().expect("读得了").len(),
        made,
        "解析不出的什么都不造"
    );
}

/// 事件流订阅的回应也带钉的看图模型（施工 CT-1补，终端照它画会话设置）：没钉的不写；钉了以后、核心重启照日志载入以后都是
/// `c/w`。带不带 `after` 都一样。
#[tokio::test]
async fn the_events_subscription_carries_the_pinned_vision_model() {
    let home = Home::new();
    let (first, _servers) = three(&home, 0).await;
    let mut client = client(&first).await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert!(reply["result"].get("vision").is_none(), "{reply}");
    let reply = pin(&mut client, "c3", &session, json!("c/w")).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reply = client.subscribe("c4", &session).await;
    assert_eq!(reply["result"]["vision"], "c/w", "{reply}");
    first.stop_sessions().await;
    drop(client);
    let again = routed(&home, &[], data(profiles(json!({}))));
    let mut restarted = Client::connect(again);
    restarted.hello().await;
    let reply = restarted.subscribe("c5", &session).await;
    assert_eq!(reply["result"]["vision"], "c/w", "重启以后照日志：{reply}");
    let (_, reply) = restarted.subscribe_after("c6", &session, json!(0)).await;
    assert_eq!(reply["result"]["vision"], "c/w", "带 after 的：{reply}");
}
