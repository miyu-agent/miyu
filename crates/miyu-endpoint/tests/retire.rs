//! 闲够了的会话退下（施工 V-2 再补，`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条）：会话表隔一阵问一遍，没人订阅、
//! 闲够了的从表里拿掉；再说话照常载入回来接着用。订阅着的不退，放下订阅以后再闲够了才退。封存了的支线（施工 V-51）判完、
//! 退订就当场从表里拿掉，不等隔一阵问；再订阅照常载入，补发的和退下以前收到的逐字一样，再退订又当场退下。

use std::time::Duration;

use serde_json::json;

use crate::support::{Client, Home, TOKEN};
use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

/// 测试里闲多久退下。
const IDLE: Duration = Duration::from_millis(200);

/// 等到载入着 `count` 个会话，最多六十秒。
async fn until_loaded(core: &Core, count: usize) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while core.loaded().await != count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内载入着的没变成 {count} 个");
}

/// 闲 [`IDLE`] 退下的核心。剧本要排上标题：没排的，假模型不回起标题的请求，会话一直有事。
fn core(home: &Home, script: &Script) -> std::sync::Arc<Core> {
    core_idle(home, script, IDLE)
}

/// 同 [`core`]，闲 `idle` 退下。
fn core_idle(home: &Home, script: &Script, idle: Duration) -> std::sync::Arc<Core> {
    std::sync::Arc::new(
        home.core_full(script, Catalog::default(), None, TOKEN)
            .with_sandbox(miyu_sandbox::Availability::Usable("miyu-sandbox".into()))
            .with_session_idle(idle),
    )
}

#[tokio::test]
async fn an_idle_session_leaves_the_table_and_comes_back_when_spoken_to() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。"), Play::Says("还在。")]);
    let core = core(&home, &script.titles([Play::Says("测试")]));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    client.say("m1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    until_loaded(&core, 0).await;
    let reply = client.say("m2", &session, "还在吗").await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    assert_eq!(core.loaded().await, 1, "说话时载入回来");
    until_loaded(&core, 0).await;
    drop(client);
    // 核心重启以后，先载入（不是先造）的会话也照样退下。
    let script = Script::new([Play::Says("又在。")]).titles([Play::Says("测试")]);
    let core = self::core(&home, &script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    client.say("m3", &session, "又来了").await;
    home.until_turns(&session, 3).await;
    until_loaded(&core, 0).await;
}

#[tokio::test]
async fn a_subscribed_session_stays_until_the_head_lets_go() {
    let home = Home::new();
    let core = core(
        &home,
        &Script::new([Play::Says("在。")]).titles([Play::Says("测试")]),
    );
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    let subscribed = client.subscribe("s1", &session).await;
    assert!(subscribed.get("error").is_none(), "{subscribed}");
    client.say("m1", &session, "在吗").await;
    client.until_turn_ends(&session).await;
    tokio::time::sleep(IDLE * 5).await;
    assert_eq!(core.loaded().await, 1, "订阅着，不退");
    drop(client);
    until_loaded(&core, 0).await;
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let listed = client.call("l1", "session.list", json!({})).await;
    let sessions = listed["result"]["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        sessions
            .iter()
            .any(|item| item["session"] == json!(session)),
        "退下的会话照样列得出：{listed}"
    );
}

/// 封存了的支线（施工 V-51）：会话表隔一小时才问一遍，支线跑完一轮封存、退订，当场从表里拿掉（主会话还在）；再订阅（从头补发）
/// 载入回来，补发的和退下以前订阅着收到的逐字一样；再退订又当场退下。
#[tokio::test]
async fn a_sealed_branch_leaves_as_soon_as_it_is_done_and_reads_back_the_same() {
    let home = Home::new();
    let script =
        Script::new([Play::Says("在。"), Play::Says("支线。")]).titles([Play::Says("测试")]);
    let core = core_idle(&home, &script, Duration::from_secs(3600));
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "/work").await;
    client.say("m1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let forked = client
        .call(
            "f1",
            "session.fork",
            json!({"session": session, "seal_after": 1}),
        )
        .await;
    let child = forked["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{forked}"))
        .to_string();
    let subscribed = client.subscribe("s1", &child).await;
    assert!(subscribed.get("error").is_none(), "{subscribed}");
    let reply = client.say("m2", &child, "再来").await;
    assert!(reply.get("error").is_none(), "{reply}");
    let mut live = client.until_turn_ends(&child).await;
    live.push(client.next().await.expect("没断开"));
    let live: Vec<serde_json::Value> = live
        .into_iter()
        .filter(|pushed| pushed["params"]["session"] == json!(child))
        .map(|pushed| pushed["params"]["event"].clone())
        .filter(|event| event.get("seq").is_some())
        .collect();
    assert_eq!(
        live.last().map(|event| event["kind"].clone()),
        Some(json!("session.sealed"))
    );
    assert_eq!(core.loaded().await, 2);
    let gone = client
        .call(
            "u1",
            "unsubscribe",
            json!({"session": child, "stream": "events"}),
        )
        .await;
    assert!(gone.get("error").is_none(), "{gone}");
    until_loaded(&core, 1).await;
    let (backlog, again) = client.subscribe_after("s2", &child, json!(0)).await;
    assert!(again.get("error").is_none(), "{again}");
    assert_eq!(core.loaded().await, 2, "订阅时载入回来");
    let first = live[0]["seq"].clone();
    let replayed: Vec<serde_json::Value> = backlog
        .into_iter()
        .filter(|pushed| pushed["params"]["session"] == json!(child))
        .map(|pushed| pushed["params"]["event"].clone())
        .skip_while(|event| event["seq"] != first)
        .collect();
    assert_eq!(replayed, live, "补发的和退下以前收到的逐字一样");
    client
        .call(
            "u2",
            "unsubscribe",
            json!({"session": child, "stream": "events"}),
        )
        .await;
    until_loaded(&core, 1).await;
}
