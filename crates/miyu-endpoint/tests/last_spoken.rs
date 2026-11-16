//! 会话列表的 `last_spoken`（施工 SX-1，`protocol.md` 的 `session.list`、`store/index.md`「一行记什么」）：人最后一次发话的
//! 时刻。真核心、手拨的钟。人说一句有了这一格；之后循环、定时任务叫醒、她的回复只动 `last_active`；没人说过话的不带它；索引
//! 的行都没了，照日志补回来的一样；回合中途人插的一句也推 `sessions.changed`；场所会话里外面的人说的也算。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::schedules::{Creator, Target, Wanted, When};
use miyu_kernel::event::Body;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_store::index::FILE;
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::venues::bound_core;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 循环这个包的工具。
fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

/// `session.list` 里会话 `session` 那一项。
async fn item(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or_else(|| panic!("列出了 {session}：{reply}"))
}

/// 日志里 `by` 是人的 `message.user` 的时刻，照先后。
fn spoken(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)) && event.by.is_person())
        .map(|event| event.at.to_string())
        .collect()
}

#[tokio::test]
async fn wakes_and_replies_move_last_active_but_not_last_spoken() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("在。"),
        Play::Says("看过了。"),
        Play::Says("又看了。"),
        Play::Says("到点了。"),
    ]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let quiet = client.create("c0", "~").await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let said = item(&mut client, "l1", &session).await;
    let words = spoken(&home, &session);
    assert_eq!(words.len(), 1, "{words:?}");
    assert_eq!(said["last_spoken"], json!(words[0]), "{said}");
    assert!(
        item(&mut client, "l0", &quiet)
            .await
            .get("last_spoken")
            .is_none(),
        "没人说过话的不带这一格"
    );

    // 循环马上跑一次、过 5 分钟再跑一次；定时任务到点跑一次。每一轮都是叫醒她、她回一句。
    tokio::time::sleep(Duration::from_millis(5)).await;
    let reply = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/loop 5m 看看 CI"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    hand.advance(300_000);
    home.until_turns(&session, 3).await;
    let once = Wanted {
        by: Creator::Person,
        when: When::At {
            at: Timestamp::from_unix_millis(START + 400_000).expect("在范围里"),
        },
        prompt: "到点提醒".to_string(),
        target: Target::Session {
            session: miyu_kernel::id::SessionId::parse(&session).expect("合写法"),
        },
        looping: None,
        first: None,
    };
    core.schedules().add(once).expect("建得了");
    hand.advance(100_000);
    home.until_turns(&session, 4).await;

    let woke = item(&mut client, "l2", &session).await;
    assert_eq!(spoken(&home, &session), words, "叫醒的那几轮没有人说的话");
    assert_eq!(woke["last_spoken"], said["last_spoken"], "{woke}");
    assert_ne!(woke["last_active"], said["last_active"], "动静照样往后走");
    let last = home.log(&session).last().expect("有日志").at.to_string();
    assert_eq!(woke["last_active"], json!(last));

    // 索引的行都没了：照日志补回来，那一格一样。
    rusqlite::Connection::open(home.root.index().join(FILE))
        .expect("打得开")
        .execute("DELETE FROM sessions", [])
        .expect("删得了");
    let rebuilt = item(&mut client, "l3", &session).await;
    assert_eq!(rebuilt["last_spoken"], said["last_spoken"], "{rebuilt}");
    assert!(
        item(&mut client, "l4", &quiet)
            .await
            .get("last_spoken")
            .is_none(),
        "照日志补的也不带"
    );
}

/// 回合中途人插一句：没有 `turn.started`，照样推那一项，带新的 `last_spoken`。
#[tokio::test]
async fn a_word_said_mid_turn_is_pushed() {
    let home = Home::new();
    let core = home.core(&Script::new([Play::Holds]));
    let mut watcher = Client::connect(core.clone());
    watcher.hello().await;
    let reply = watcher
        .call("w1", "subscribe", json!({"stream": "sessions"}))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "先做这个").await;
    let first = until_spoken(&mut watcher, &session, |at| at.is_some()).await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    client.say("s2", &session, "顺便看看那个").await;
    let second = until_spoken(&mut watcher, &session, |at| {
        at.is_some_and(|at| at != first)
    })
    .await;
    assert_eq!(spoken(&home, &session).last(), Some(&second));
    assert_eq!(
        home.log(&session)
            .iter()
            .filter(|event| matches!(event.body, Body::TurnStarted(_)))
            .count(),
        1,
        "插的那一句没开新的一轮"
    );
}

/// 读推送，直到会话 `session` 那一项的 `last_spoken` 合 `wanted`，交回它。
async fn until_spoken(
    watcher: &mut Client,
    session: &str,
    wanted: impl Fn(Option<&str>) -> bool,
) -> String {
    loop {
        let next = watcher.next().await.expect("推了");
        if next["method"] != "sessions.changed" || next["params"]["session"] != json!(session) {
            continue;
        }
        let at = next["params"]["entry"]["last_spoken"].as_str();
        if wanted(at) {
            return at.unwrap_or_default().to_string();
        }
    }
}

/// 场所会话里外面的人旁听地说一句：不开回合，那一格照样跟着它。
#[tokio::test]
async fn a_venue_session_follows_whoever_speaks_there() {
    let home = Home::new();
    let mut client = Client::connect(bound_core(&home, &Script::new([]), Catalog::default()));
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
    let send = json!({"session": session, "text": "在吗", "as": {"external": "qq:10001"},
        "venue": {"msg": "8810", "ambient": true}});
    let reply = client.call("s1", "session.send", send).await;
    assert!(reply.get("error").is_none(), "{reply}");
    let words = spoken(&home, &session);
    assert_eq!(words.len(), 1, "{words:?}");
    // 场所会话不进本机的会话列表：看索引里那一行（列会话照它算）。
    let listed = client.call("l1", "session.list", json!({})).await;
    assert!(
        listed["result"]["sessions"]
            .as_array()
            .is_some_and(|all| all.iter().all(|item| item["session"] != json!(session))),
        "{listed}"
    );
    let db = rusqlite::Connection::open_with_flags(
        home.root.index().join(FILE),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("打得开");
    let at: Option<i64> = db
        .query_row(
            "SELECT last_spoken FROM sessions WHERE id = ?1",
            [&session],
            |row| row.get(0),
        )
        .expect("有这一行");
    let at = at
        .and_then(Timestamp::from_unix_millis)
        .map(|at| at.to_string());
    assert_eq!(at.as_ref(), words.first(), "外面的人说的也算");
}
