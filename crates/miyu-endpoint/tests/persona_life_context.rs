//! 人生会话的上下文上限（施工 LF-10，设计 `70-推演人生.md` 第四节，`docs/blueprint/life.md`「上下文上限」）：真核心、手拨的钟。
//! 人格的 `[life] context` 经 `persona.set` 的 `changes` 写、删，`persona.get` 带着它和出厂的数；`life.start` 照它设（没写的
//! 100000，太小的抬到最少要的）；`persona.set` 确实改了它的，当场改这个人格没结束的人生会话的上限，一样的不多记；到点不同步，
//! 人生会话里 `/context` 改的一直管用，直到再在人格里改。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时。
const GAP: i64 = 3_600_000;

/// 出厂的数。
const SHIPPED: u64 = 100_000;

/// 起好唤醒表的核心、握好手的连接、拨钟的手；人格 `muse` 写在数据根里，`persona.toml` 照 `toml` 写（空的不写）。
async fn started(home: &Home, script: &Script, toml: &str) -> (Arc<Core>, Client, Arc<Hand>) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    if !toml.is_empty() {
        home.write("root/personas/muse/persona.toml", toml);
    }
    let hand = Hand::at(START);
    let core = core_with_tools(home, script, &hand, Catalog::default());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client, hand)
}

/// `life.start` 开出来的会话编号，等第一次醒完。
async fn start(home: &Home, client: &mut Client, id: &str) -> String {
    let reply = client
        .call(id, "life.start", json!({"persona": "muse"}))
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string();
    home.until_turns(&session, 1).await;
    session
}

/// 日志里设上限的事件（`session.policy_changed` 带 `context` 的），照先后，各是设成的数。
fn caps(log: &[Event]) -> Vec<Option<u64>> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::PolicyChanged(changed) => changed.context.as_ref().map(|cap| cap.tokens),
            _ => None,
        })
        .collect()
}

/// 改 `life.context`：`Some` 是写这个数，`None` 是删掉。
async fn set_life(client: &mut Client, id: &str, tokens: Option<Value>) -> Value {
    let change = match tokens {
        Some(value) => json!({"key": "life.context", "value": value}),
        None => json!({"key": "life.context", "unset": true}),
    };
    client
        .call(
            id,
            "persona.set",
            json!({"persona": "muse", "changes": [change]}),
        )
        .await
}

/// 拒绝的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

/// 家目录那一层的 `persona.toml`，没有的是空的。
fn mine(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("root/personas/muse/persona.toml"))
        .unwrap_or_default()
}

#[tokio::test]
async fn life_context_is_set_and_unset_through_changes() {
    let home = Home::new();
    let (_core, mut client, _hand) = started(&home, &Script::new([]), "").await;
    let got = client
        .call("g1", "persona.get", json!({"persona": "muse"}))
        .await;
    assert_eq!(got["result"]["life_context"], json!(null), "{got}");
    assert_eq!(got["result"]["life_context_default"], json!(SHIPPED));

    let saved = set_life(&mut client, "s1", Some(json!(60_000))).await;
    assert_eq!(saved["result"]["life_context"], json!(60_000), "{saved}");
    assert_eq!(saved["result"]["life_context_default"], json!(SHIPPED));
    assert!(mine(&home).contains("context = 60000"), "{}", mine(&home));
    let got = client
        .call("g2", "persona.get", json!({"persona": "muse"}))
        .await;
    assert_eq!(got["result"]["life_context"], json!(60_000), "{got}");

    for (n, wrong) in [json!(0), json!(-5), json!(1.5), json!("a")]
        .into_iter()
        .enumerate()
    {
        let refused = set_life(&mut client, &format!("w{n}"), Some(wrong.clone())).await;
        assert_eq!(reason(&refused), "persona_invalid", "{wrong}: {refused}");
        assert!(
            mine(&home).contains("context = 60000"),
            "{wrong}：什么都没写"
        );
    }

    let unset = set_life(&mut client, "u1", None).await;
    assert_eq!(unset["result"]["life_context"], json!(null), "{unset}");
    assert!(!mine(&home).contains("context"), "{}", mine(&home));
}

#[tokio::test]
async fn life_start_caps_by_the_persona() {
    for (toml, wanted) in [
        ("", Some(SHIPPED)),
        ("[life]\ncontext = 50000\n", Some(50_000)),
        ("[life]\ncontext = 1\n", None),
    ] {
        let home = Home::new();
        let script = Script::new([Play::Says("醒了。")]);
        let (_core, mut client, _hand) = started(&home, &script, toml).await;
        let session = start(&home, &mut client, "l1").await;
        let wanted = match wanted {
            Some(wanted) => wanted,
            // 比最少要的还小的抬到最少要的：最少要的照 `session.configure` 拒 1 时说的 `min`。
            None => {
                let refused = client
                    .call(
                        "c1",
                        "session.configure",
                        json!({"session": session, "context": 1}),
                    )
                    .await;
                let least = refused["error"]["data"]["min"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("说了最少要多少：{refused}"));
                assert!(least > 1, "{least}");
                least
            }
        };
        assert_eq!(caps(&home.log(&session)), [Some(wanted)], "{toml:?}");
    }
}

#[tokio::test]
async fn persona_set_moves_a_running_or_paused_life_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let (_core, mut client, _hand) = started(&home, &script, "").await;
    let session = start(&home, &mut client, "l1").await;
    assert_eq!(caps(&home.log(&session)), [Some(SHIPPED)]);

    let saved = set_life(&mut client, "s1", Some(json!(60_000))).await;
    assert!(saved.get("error").is_none(), "{saved}");
    assert_eq!(
        caps(&home.log(&session)),
        [Some(SHIPPED), Some(60_000)],
        "当场换成新的，只多一条"
    );
    // 同一个数再写一次：人格没变，会话也不多记。
    let again = set_life(&mut client, "s2", Some(json!(60_000))).await;
    assert!(again.get("error").is_none(), "{again}");
    assert_eq!(caps(&home.log(&session)).len(), 2, "一样的不多记");

    let paused = client
        .call("p1", "life.pause", json!({"persona": "muse"}))
        .await;
    assert_eq!(paused["result"]["state"], "paused", "{paused}");
    let unset = set_life(&mut client, "u1", None).await;
    assert!(unset.get("error").is_none(), "{unset}");
    assert_eq!(
        caps(&home.log(&session)),
        [Some(SHIPPED), Some(60_000), Some(SHIPPED)],
        "停着的也换；删掉的换回出厂的数"
    );
}

#[tokio::test]
async fn persona_set_leaves_no_life_and_ended_lives_alone() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let (_core, mut client, _hand) = started(&home, &script, "").await;
    // 没有人生会话的：照样写成。
    let saved = set_life(&mut client, "s1", Some(json!(60_000))).await;
    assert_eq!(saved["result"]["life_context"], json!(60_000), "{saved}");

    let session = start(&home, &mut client, "l1").await;
    assert_eq!(
        caps(&home.log(&session)),
        [Some(60_000)],
        "开的时候照人格写的"
    );
    let killed = client
        .call("k1", "life.kill", json!({"persona": "muse"}))
        .await;
    assert_eq!(killed["result"]["state"], "ended", "{killed}");
    let before = home.log(&session).len();
    let saved = set_life(&mut client, "s2", Some(json!(70_000))).await;
    assert_eq!(saved["result"]["life_context"], json!(70_000), "{saved}");
    assert_eq!(home.log(&session).len(), before, "结束了的什么都不做");
}

#[tokio::test]
async fn context_typed_in_life_stays_across_wakeups() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        Play::Says("又醒了。"),
        Play::Says("还醒着。"),
    ]);
    let (_core, mut client, hand) = started(&home, &script, "").await;
    let session = start(&home, &mut client, "l1").await;
    let typed = client
        .call(
            "t1",
            "command.run",
            json!({"session": session, "text": "/context 70000"}),
        )
        .await;
    assert!(typed.get("error").is_none(), "{typed}");
    assert_eq!(caps(&home.log(&session)), [Some(SHIPPED), Some(70_000)]);

    // 改别的格的 `persona.set` 不碰人生会话：上限还是 `/context` 改的。
    let renamed = client
        .call(
            "n1",
            "persona.set",
            json!({"persona": "muse", "changes": [{"key": "persona.name", "value": "缪斯"}]}),
        )
        .await;
    assert!(renamed.get("error").is_none(), "{renamed}");
    assert_eq!(
        caps(&home.log(&session)).last(),
        Some(&Some(70_000)),
        "改名字不动上限"
    );

    // 到点不同步：再醒两次，上限还是 `/context` 改的。
    hand.advance(GAP);
    home.until_turns(&session, 2).await;
    hand.advance(GAP);
    home.until_turns(&session, 3).await;
    assert_eq!(
        caps(&home.log(&session)),
        [Some(SHIPPED), Some(70_000)],
        "醒几次都还是 /context 改的"
    );

    // 再在人格里改：换成人格里新写的。
    let saved = set_life(&mut client, "s1", Some(json!(55_000))).await;
    assert!(saved.get("error").is_none(), "{saved}");
    assert_eq!(caps(&home.log(&session)).last(), Some(&Some(55_000)));
}
