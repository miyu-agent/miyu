//! 人生的两样小账（施工 LF-14，`docs/blueprint/life.md`「推演轮数」「结束了、还没看过」）：真核心、手拨的钟。
//!
//! - 推演轮数：每次到点交进会话算一轮，`life.status`、`life.list`、`schedule.list` 的人生那一项带 `rounds`；停着不加，续上
//!   那一次算一轮。
//! - 结束了、还没看过：她自己结束的记一条，`life.list` 的 `ended_unseen` 列它，通知 `life.ended` 带人格的名字；有头打开那个
//!   会话、头说看过了、会话删了就清掉，清掉推 `life.seen`。主人结束的、结束那一刻有头正开着的不记。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::life_command::asleep;
use crate::life_self_end::{end_life, notice, start, started};
use crate::support::clock::Hand;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时。
const GAP: i64 = 3_600_000;

/// `life.list` 的回应。
async fn listed(client: &mut Client, id: &str) -> Value {
    client.call(id, "life.list", json!({})).await["result"].clone()
}

/// `life.status` 的 `rounds`。
async fn rounds(client: &mut Client, id: &str) -> Value {
    client
        .call(id, "life.status", json!({"persona": "muse"}))
        .await["result"]["rounds"]
        .clone()
}

/// 等到通知 `kind`，交回它；别的通知跳过。
async fn told(client: &mut Client, kind: &str) -> Value {
    loop {
        let one = notice(client, Duration::from_secs(30))
            .await
            .unwrap_or_else(|| panic!("等得到 {kind}"));
        if one["kind"] == kind {
            return one;
        }
    }
}

/// 每次到点交进会话算一轮：开出来马上醒的是第 1 轮，到点再醒第 2 轮；停着的不加，续上马上醒的那一次第 3 轮。三处都带。
#[tokio::test]
async fn each_wake_delivered_is_one_round() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        Play::Says("又醒了。"),
        Play::Says("第三次。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    asleep(&mut client, &session).await;
    assert_eq!(rounds(&mut client, "r1").await, 1);

    hand.advance(GAP + 1_000);
    home.until_turns(&session, 2).await;
    asleep(&mut client, &session).await;
    assert_eq!(rounds(&mut client, "r2").await, 2);
    assert_eq!(listed(&mut client, "q1").await["lives"][0]["rounds"], 2);
    let table = client.call("q2", "schedule.list", json!({})).await;
    assert_eq!(table["result"]["schedules"][0]["rounds"], 2, "{table}");

    let paused = client
        .call("p1", "life.pause", json!({"persona": "muse"}))
        .await;
    assert_eq!(paused["result"]["rounds"], 2, "{paused}");
    hand.advance(3 * GAP);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(rounds(&mut client, "r3").await, 2, "停着不加");
    client
        .call("p2", "life.resume", json!({"persona": "muse"}))
        .await;
    home.until_turns(&session, 3).await;
    assert_eq!(rounds(&mut client, "r4").await, 3, "续上那一次算一轮");
}

/// 她自己结束的：通知带人格的名字；`life.list` 的 `ended_unseen` 列它，`lives` 里没有了。另一个头打开那个会话就清掉，推
/// `life.seen`。
#[tokio::test]
async fn a_life_she_ended_stays_unseen_until_someone_opens_it() {
    let home = Home::new();
    home.write(
        "root/personas/muse/persona.toml",
        "[persona]\nname = \"缪斯\"\n",
    );
    let script = Script::new([end_life("再见。"), Play::Says("我走了。")]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    let session = start(&mut client, "l1").await;
    let ended = told(&mut client, "life.ended").await;
    assert_eq!(ended["name"], "缪斯", "{ended}");
    assert_eq!(ended["persona"], "muse", "{ended}");

    let now = listed(&mut client, "q1").await;
    assert_eq!(now["lives"], json!([]), "{now}");
    let unseen = now["ended_unseen"].as_array().expect("有这一格");
    assert_eq!(unseen.len(), 1, "{now}");
    assert_eq!(unseen[0]["session"], json!(session), "{now}");
    assert_eq!(unseen[0]["persona"], "muse", "{now}");
    assert_eq!(unseen[0]["name"], "缪斯", "{now}");
    assert!(unseen[0]["at"].is_string(), "{now}");

    let mut other = Client::connect(std::sync::Arc::clone(&core));
    other.hello().await;
    let opened = other
        .call("v1", "session.viewing", json!({"session": session}))
        .await;
    assert_eq!(opened["result"], json!({}), "{opened}");
    let seen = told(&mut client, "life.seen").await;
    assert_eq!(seen["session"], json!(session), "{seen}");
    assert_eq!(listed(&mut client, "q2").await["ended_unseen"], json!([]));
}

/// 主人结束的不记；她结束的那一刻有头正开着这个会话的也不记。
#[tokio::test]
async fn ended_by_the_owner_or_while_watched_is_not_unseen() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let killed = client
        .call("k1", "life.kill", json!({"persona": "muse"}))
        .await;
    assert_eq!(killed["result"]["state"], "ended", "{killed}");
    assert_eq!(listed(&mut client, "q1").await["ended_unseen"], json!([]));

    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        end_life("再见。"),
        Play::Says("我走了。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    asleep(&mut client, &session).await;
    client
        .call("v1", "session.viewing", json!({"session": session}))
        .await;
    hand.advance(GAP + 1_000);
    told(&mut client, "life.ended").await;
    assert_eq!(listed(&mut client, "q1").await["ended_unseen"], json!([]));
}

/// 头说一声看过了（`life.seen`）清掉，再说一次照样回 `{}`；删了那个会话的也清掉。都只清那一个会话的。
#[tokio::test]
async fn saying_seen_or_deleting_the_session_clears_it() {
    for clear in ["life.seen", "session.delete"] {
        let home = Home::new();
        let script = Script::new([end_life("再见。"), Play::Says("我走了。")]);
        let hand = Hand::at(START);
        let (_core, mut client) = started(&home, &script, &hand).await;
        client
            .call("n", "subscribe", json!({"stream": "notices"}))
            .await;
        let session = start(&mut client, "l1").await;
        told(&mut client, "life.ended").await;
        let unseen = listed(&mut client, "q1").await["ended_unseen"].clone();
        assert_eq!(
            unseen.as_array().map(Vec::len),
            Some(1),
            "{clear}：{unseen}"
        );

        let reply = client.call("c1", clear, json!({"session": session})).await;
        assert_eq!(reply["result"], json!({}), "{clear}：{reply}");
        assert_eq!(
            listed(&mut client, "q2").await["ended_unseen"],
            json!([]),
            "{clear}"
        );
        if clear == "life.seen" {
            let again = client.call("c2", clear, json!({"session": session})).await;
            assert_eq!(
                again["result"],
                json!({}),
                "本来就没记着的也回 {{}}：{again}"
            );
            let bad = client
                .call("c3", clear, json!({"session": "不是编号"}))
                .await;
            assert_eq!(bad["error"]["data"]["reason"], "bad_params", "{bad}");
        }
    }
}
