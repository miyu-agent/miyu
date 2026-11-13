//! 人生循环（施工 LF-1，设计 `70-推演人生.md` 第三节第 2 条）：真核心、手拨的钟。和她自己定节奏的循环走同一段代码，只有三处
//! 不同：交过以后没定下一次的照默认间隔再醒，她在那一轮里定了的照她的；核心起来时错过的马上补醒一次（停了多久都只一次）；
//! `stop` 不收。通用的定时入口（`schedule.*`、`schedules` 流、`cron_list`、`cron_delete`）碰不到这一行。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, ScheduleLoop};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时（`schedules.md` 第三节第 2 条）。
const GAP: i64 = 3_600_000;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

/// 跑在 `session` 上、种类是 `looping` 的一行，第一次在 `first`。
fn looping(session: &str, looping: Loop, first: i64) -> Wanted {
    Wanted {
        by: Creator::Person,
        when: When::At { at: at(first) },
        prompt: "过日子".to_string(),
        target: Target::Session {
            session: SessionId::parse(session).expect("会话编号合写法"),
        },
        looping: Some(looping),
        first: Some(at(first)),
    }
}

/// 定时、循环两个包的工具。
fn catalog() -> Catalog {
    let resources = default_resources();
    Catalog::in_packages([
        (
            miyu_schedule::SCHEDULE,
            miyu_schedule::schedule_tools(&resources).expect("出厂的字"),
        ),
        (
            miyu_schedule::LOOP,
            miyu_schedule::loop_tools(&resources).expect("出厂的字"),
        ),
    ])
    .expect("合写法")
}

/// 起好唤醒表的核心（工具目录是 `tools`）、握好手的连接、开好的一个会话。
async fn started(
    home: &Home,
    script: &Script,
    hand: &Arc<Hand>,
    tools: Catalog,
) -> (Arc<Core>, Client, String) {
    let core = core_with_tools(home, script, hand, tools);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    core.start_schedules();
    (core, client, session)
}

/// 等到编号是 `id` 的那一个记了上一次（最多五秒），交回它。
async fn until_recorded(core: &Core, id: &str) -> Schedule {
    let waited = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let schedule = core.schedules().get(id).expect("有这一个");
            if schedule.last.is_some() {
                return schedule;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| panic!("{id} 没跑"))
}

/// 日志里定时交进来的话的 `by`。
fn scheduled(log: &[Event]) -> Vec<miyu_kernel::origin::Schedule> {
    log.iter()
        .filter_map(|event| match (&event.by, &event.body) {
            (By::Schedule(by), Body::MessageUser(_)) => Some(by.clone()),
            _ => None,
        })
        .collect()
}

/// 拒绝的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

#[tokio::test]
async fn after_it_speaks_a_life_loop_falls_back_to_the_default_gap() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, _client, session) = started(&home, &script, &hand, Catalog::default()).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, START))
        .unwrap();
    home.until_turns(&session, 1).await;
    let after = until_recorded(&core, &added.id).await;
    assert_eq!(after.next, Some(at(START + GAP)), "没定的照默认间隔再醒");
    let said = scheduled(&home.log(&session));
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(said[0].looping, Some(ScheduleLoop::Life));
}

#[tokio::test]
async fn her_wakeup_in_the_woken_turn_beats_the_default() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("schedule_wakeup", r#"{"delay_seconds":600}"#)]),
        Play::Says("十分钟后再看。"),
    ]);
    let hand = Hand::at(START);
    let (core, _client, session) = started(&home, &script, &hand, catalog()).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, START))
        .unwrap();
    home.until_turns(&session, 1).await;
    let after = until_recorded(&core, &added.id).await;
    assert_eq!(after.last.unwrap().result, "ran");
    assert_eq!(
        after.next,
        Some(at(START + 600_000)),
        "她定的十分钟，不是默认的一小时"
    );
}

#[tokio::test]
async fn a_missed_life_loop_wakes_once_on_restart_and_a_dynamic_one_waits() {
    let home = Home::new();
    let hand = Hand::at(START);
    let session = {
        let core = core_with(&home, &Script::new([]), &hand);
        let mut client = Client::connect(Arc::clone(&core));
        client.hello().await;
        let cwd = home.work.to_string_lossy().into_owned();
        let session = client.create("c1", &cwd).await;
        let schedules = core.schedules();
        schedules
            .add(looping(&session, Loop::Life, START + 60_000))
            .unwrap();
        schedules
            .add(looping(&session, Loop::Dynamic, START + 60_000))
            .unwrap();
        session
    };
    // 停了三天：错过的照一分钟一次算有几千次，人生只补一次。
    let gone = 3 * 24 * GAP;
    hand.advance(gone);
    let script = Script::new([Play::Says("醒了。"), Play::Says("又醒了。")]);
    let core = core_with(&home, &script, &hand);
    core.start_schedules();
    home.until_turns(&session, 1).await;
    let life = until_recorded(&core, "t1").await;
    assert_eq!(
        life.next,
        Some(at(START + gone + GAP)),
        "补过以后照现在加默认间隔"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let said = scheduled(&home.log(&session));
    assert_eq!(said.len(), 1, "只补一次：{said:?}");
    assert_eq!(said[0].schedule, "t1");
    let dynamic = core.schedules().get("t2").unwrap();
    assert_eq!(dynamic.next, None, "她自己定节奏的照旧停着等");
    assert!(dynamic.last.is_none(), "没交");
}

#[tokio::test]
async fn her_schedule_tools_cannot_stop_list_or_delete_a_life_loop() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("schedule_wakeup", r#"{"stop":true}"#)]),
        Play::calls(&[("cron_delete", r#"{"id":"t1"}"#)]),
        Play::calls(&[("cron_list", "{}")]),
        Play::Says("好。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &script, &hand, catalog()).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, START + GAP))
        .unwrap();
    client.say("s1", &session, "安排一下").await;
    home.until_turns(&session, 1).await;
    let row = core.schedules().get(&added.id).expect("那一行还在");
    assert_eq!(row, added, "下一次没动");
    let log = format!("{:?}", home.log(&session));
    assert!(log.contains("wakeup/unstoppable"), "stop 不收：{log}");
    assert!(log.contains("cron/not-found"), "删不了，照没有回：{log}");
    assert!(!log.contains("过日子"), "列表里没有它：{log}");
}

#[tokio::test]
async fn schedule_methods_cannot_reach_a_life_loop() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (core, mut client, session) =
        started(&home, &Script::new([]), &hand, Catalog::default()).await;
    let added = core
        .schedules()
        .add(looping(&session, Loop::Life, START + GAP))
        .unwrap();
    let listed = client.call("l", "schedule.list", json!({})).await;
    assert_eq!(listed["result"], json!({"schedules": []}), "{listed}");
    for (method, (name, params)) in [
        ("schedule.cancel", json!({"id": "t1"})),
        ("schedule.pause", json!({"id": "t1"})),
        ("schedule.resume", json!({"id": "t1"})),
        (
            "schedule.update",
            json!({"id": "t1", "when": {"at": at(START + 2 * GAP)}}),
        ),
        ("schedule.update", json!({"id": "t1", "prompt": "别过了"})),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("m{method}"), name, params).await;
        assert_eq!(reason(&reply), "unknown_schedule", "{name}：{reply}");
    }
    assert_eq!(
        core.schedules().get(&added.id).unwrap(),
        added,
        "那一行不动"
    );

    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    let reply = watcher
        .call("s", "subscribe", json!({"stream": "schedules"}))
        .await;
    assert_eq!(reply["result"], json!({"schedules": []}), "{reply}");
    // 改了人生那一行不推；接着建的一行推出来的是它自己。
    core.schedules()
        .set_next(&added.id, Some(at(START + 600_000)), None)
        .unwrap();
    client
        .call(
            "a",
            "schedule.add",
            json!({"when": {"every": 600}, "prompt": "看看", "target": {"session": session}}),
        )
        .await;
    let pushed = loop {
        let message = watcher
            .next_within(Duration::from_secs(60))
            .await
            .expect("等得到推送");
        if message["method"] == "schedules.changed" {
            break message["params"].clone();
        }
    };
    assert_eq!(pushed["id"], "t2", "人生那一行不推：{pushed}");
}

/// 拦人生那一行不挡人删读不懂的行（`schedules.md` 第一节第 3 条，施工 AE-4）：`get` 读不出的放过去，`schedule.cancel` 照旧
/// 删得掉。
#[tokio::test]
async fn a_person_still_cancels_an_unreadable_row_over_the_protocol() {
    let home = Home::new();
    let hand = Hand::at(START);
    let (core, mut client, session) =
        started(&home, &Script::new([]), &hand, Catalog::default()).await;
    let bad = core
        .schedules()
        .add(looping(&session, Loop::Life, START + GAP))
        .unwrap();
    rusqlite::Connection::open(miyu_store::schedules::path(&home.root))
        .expect("开得了")
        .execute(
            "UPDATE schedules SET when_json = '{\"fortnightly\":true}' WHERE id = ?1",
            [&bad.id],
        )
        .expect("改得了");
    assert!(core.schedules().get(&bad.id).is_err(), "照编号读不出");
    let reply = client
        .call("x", "schedule.cancel", json!({"id": bad.id}))
        .await;
    assert_eq!(reply["result"], json!({}), "删得掉：{reply}");
    let again = client
        .call("x2", "schedule.cancel", json!({"id": bad.id}))
        .await;
    assert_eq!(reason(&again), "unknown_schedule", "那一行没了：{again}");
}
