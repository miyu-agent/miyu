//! 结束一段人生（施工 LF-4 上，设计 `70-推演人生.md` 第三节第 5、6 条，`docs/blueprint/life.md`「结束」）：真核心、手拨的钟。
//! `life.kill` 记 `life.ended`、封存、删人生那一行、清这段人生的记忆和回合索引、推一条通知；之后不再醒、发话拒；状态是
//! `ended`，停、续拒 `life_ended`，开、`life.new` 开出新的一条；再结束一次原样成功、不再推；结束到一半留下的那一行到点删掉、
//! 不叫醒她。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_store::index::FILE;
use miyu_store::recall::{RecallIndexes, Room};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时。
const GAP: i64 = 3_600_000;

/// 起好唤醒表、带记忆三件工具的核心，握好手的连接。人格 `muse` 写在数据根里。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let resources = default_resources();
    // 照包登记：预设「推演人生」的 `unlisted = "off"`，不归哪个包的工具给不了。
    let memory = miyu_memory::tools(&resources).expect("读得出");
    let catalog = Catalog::in_packages(vec![("base-memory", memory)]).expect("合写法");
    let core = core_with_tools(home, script, hand, catalog);
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client)
}

fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

async fn start(client: &mut Client, id: &str) -> String {
    let reply = client
        .call(id, "life.start", json!({"persona": "muse"}))
        .await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string()
}

async fn kill(client: &mut Client, id: &str) -> Value {
    client
        .call(id, "life.kill", json!({"persona": "muse"}))
        .await
}

fn life_row(core: &Core, session: &str) -> Option<Schedule> {
    core.schedules()
        .list()
        .expect("读得了")
        .into_iter()
        .find(|one| {
            one.looping == Some(Loop::Life)
                && matches!(&one.target, Target::Session { session: on } if on.as_str() == session)
        })
}

/// 人生循环叫醒了几次。
fn woken(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| {
            matches!(
                (&event.by, &event.body),
                (By::Schedule(_), Body::MessageUser(_))
            )
        })
        .count()
}

fn ended(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.body, Body::LifeEnded(_)))
        .collect()
}

/// 读到一条 `notice` 为止，等 `wait`；等不到的没有。
async fn notice(client: &mut Client, wait: Duration) -> Option<Value> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client.next_within(left).await?;
        if message["method"] == "notice" {
            return Some(message["params"].clone());
        }
    }
}

/// 人格 `muse` 记下的、还算数的正文。
async fn memories(client: &mut Client, id: &str) -> Vec<String> {
    let listed = client
        .call(id, "memory.list", json!({"persona": "muse"}))
        .await;
    listed["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .map(|one| one["text"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// `muse` 那一间回合库里会话 `session` 照到了哪：没有的是空的。
fn indexed(home: &Home, session: &str) -> Option<u64> {
    let (index, _) = RecallIndexes::new(&home.root).turns(&Room::persona("muse"));
    index
        .mark(session)
        .expect("读得了")
        .map(miyu_kernel::id::Seq::get)
}

fn remember(text: &str) -> Play {
    Play::calls(&[(
        "remember",
        &json!({"class": "user", "text": text}).to_string(),
    )])
}

#[tokio::test]
async fn kill_ends_a_life_and_clears_what_it_left() {
    let home = Home::new();
    let script = Script::new([
        remember("她今天去了海边"),
        Play::Says("记下了。"),
        remember("用户爱喝茶"),
        Play::Says("好。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let plain = client.create_as("c1", "~", "muse").await;
    client.say("c2", &plain, "我爱喝茶").await;
    home.until_turns(&plain, 1).await;
    let before = memories(&mut client, "m1").await;
    assert_eq!(before.len(), 2, "{before:?}");
    for _ in 0..200 {
        if indexed(&home, &session).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(indexed(&home, &session).is_some(), "过的那一轮进了回合索引");
    let subscribed = client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    assert_eq!(subscribed["result"], json!({}), "{subscribed}");

    let reply = kill(&mut client, "k1").await;
    assert_eq!(
        reply["result"],
        json!({"state": "ended", "session": session})
    );
    let log = home.log(&session);
    let ends = ended(&log);
    assert_eq!(ends.len(), 1, "{log:?}");
    assert!(matches!(ends[0].by, By::Person(_)), "人结束的");
    assert_eq!(ends[0].turn, None);
    assert!(
        matches!(&ends[0].body, Body::LifeEnded(body) if body.last_words.is_none()),
        "人结束的没有遗言"
    );
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::SessionSealed(_))),
        "封存了"
    );
    assert!(life_row(&core, &session).is_none(), "人生那一行删了");
    let told = notice(&mut client, Duration::from_secs(30))
        .await
        .expect("推了通知");
    assert_eq!(told["kind"], "life.ended", "{told}");
    assert_eq!(told["by"], "owner");
    assert_eq!(told["persona"], "muse");
    assert_eq!(told["session"], json!(session));
    assert!(told.get("last_words").is_none(), "{told}");
    assert_eq!(
        memories(&mut client, "m2").await,
        ["用户爱喝茶"],
        "这段人生记下的清掉，别的会话记的照旧"
    );
    assert_eq!(indexed(&home, &session), None, "回合索引里拿掉了");
    assert!(indexed(&home, &plain).is_some(), "别的会话的照旧");

    hand.advance(5 * GAP);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(woken(&home.log(&session)), 1, "往后拨多久都不再醒");
    let said = client.say("s1", &session, "还在吗").await;
    assert_eq!(reason(&said), "session_sealed", "{said}");
    let status = client
        .call("st", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(
        status["result"],
        json!({"state": "ended", "session": session})
    );

    // 会话列表那一项：`life`、`life_ended`；索引的行都没了照日志补回来一样。
    let item = |listed: &Value| {
        listed["result"]["sessions"]
            .as_array()
            .unwrap_or_else(|| panic!("{listed}"))
            .iter()
            .find(|item| item["session"] == json!(session))
            .cloned()
            .expect("列得出")
    };
    let listed = client.call("ls1", "session.list", json!({})).await;
    let first = item(&listed);
    assert_eq!(first["life"], true, "{first}");
    assert_eq!(first["life_ended"], true, "{first}");
    rusqlite::Connection::open(home.root.index().join(FILE))
        .expect("打得开")
        .execute("DELETE FROM sessions", [])
        .expect("删得了");
    let listed = client.call("ls2", "session.list", json!({})).await;
    assert_eq!(item(&listed), first, "照日志重建的一样");
}

#[tokio::test]
async fn an_ended_life_refuses_pause_and_resume_and_new_ones_start_fresh() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        Play::Says("又是新的一生。"),
        Play::Says("第三段。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let first = start(&mut client, "l1").await;
    home.until_turns(&first, 1).await;
    assert_eq!(kill(&mut client, "k1").await["result"]["state"], "ended");
    for method in ["life.pause", "life.resume"] {
        let refused = client.call("x", method, json!({"persona": "muse"})).await;
        assert_eq!(reason(&refused), "life_ended", "{method}：{refused}");
        assert_eq!(refused["error"]["data"]["session"], json!(first));
    }
    let second = start(&mut client, "l2").await;
    assert_ne!(second, first, "开出一条新的");
    home.until_turns(&second, 1).await;
    let status = client
        .call("s", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(status["result"]["state"], "running", "{status}");
    assert_eq!(status["result"]["session"], json!(second));

    // 有一条在跑的时候 `life.new`：旧的结束，新的开出来。
    let renewed = client
        .call("n1", "life.new", json!({"persona": "muse"}))
        .await;
    assert_eq!(renewed["result"]["ended"], json!(second), "{renewed}");
    let third = renewed["result"]["session"]
        .as_str()
        .expect("开出来了")
        .to_string();
    assert!(third != second && third != first);
    home.until_turns(&third, 1).await;
    assert_eq!(ended(&home.log(&second)).len(), 1, "旧的结束了");
    assert!(life_row(&core, &second).is_none());
    assert!(life_row(&core, &third).is_some());
    let status = client
        .call("s2", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(status["result"]["session"], json!(third));
}

#[tokio::test]
async fn ending_again_redoes_nothing_and_a_leftover_row_is_dropped_unwoken() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let refused = kill(&mut client, "k0").await;
    assert_eq!(reason(&refused), "no_life", "{refused}");
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    let first = kill(&mut client, "k1").await;
    assert!(
        notice(&mut client, Duration::from_secs(30)).await.is_some(),
        "第一次推"
    );
    let again = kill(&mut client, "k2").await;
    assert_eq!(again["result"], first["result"], "原样成功");
    assert_eq!(ended(&home.log(&session)).len(), 1, "不记第二条");
    assert!(
        notice(&mut client, Duration::from_millis(500))
            .await
            .is_none(),
        "不再推"
    );

    // 结束到一半断了、人生那一行还在：到点删掉，不叫醒她。
    let now = Timestamp::from_unix_millis(START + 60_000).expect("在范围里");
    core.schedules()
        .add(Wanted {
            by: Creator::Person,
            when: When::At { at: now },
            prompt: String::new(),
            target: Target::Session {
                session: SessionId::parse(&session).expect("合写法"),
            },
            looping: Some(Loop::Life),
            first: Some(now),
        })
        .expect("建得了");
    hand.advance(120_000);
    for _ in 0..200 {
        if life_row(&core, &session).is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(life_row(&core, &session).is_none(), "那一行删了");
    assert_eq!(woken(&home.log(&session)), 1, "没叫醒她");
}
