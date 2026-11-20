//! 到点的定时任务交进没人看着的一次性会话（施工 SC-1，`internal/blueprint/schedules.md` 第三节第 4 条）：真核心、手拨的钟。
//! `miyu ask` 开的会话、这时没有头订阅着：人建的到点照样开一轮，记 `ran`；她自己在里面建的只记下，上一次记 `held`，不写
//! `ran`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Schedule, Target, Wanted, When};
use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 每十分钟往 `session` 交一句的一个，`by` 建的。
fn every_ten_minutes(session: &str, by: Creator) -> Wanted {
    Wanted {
        by,
        when: When::Every { every: 600 },
        prompt: "看看 CI".to_string(),
        target: Target::Session {
            session: SessionId::parse(session).expect("会话编号合写法"),
        },
        looping: None,
        first: None,
    }
}

/// 起好唤醒表的核心、一个一次性的会话：造它的连接没订阅，没人看着。连接留着，核心不因为没人连着退出。
async fn oneshot(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client, String) {
    let core = core_with(home, script, hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with("c1", json!({"cwd": cwd, "oneshot": true}))
        .await;
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

/// 日志里定时交进来的话有几句、开了几轮。
fn said_and_turns(home: &Home, session: &str) -> (usize, usize) {
    let log = home.log(session);
    let said = log
        .iter()
        .filter(|event| {
            matches!(event.by, By::Schedule(_)) && matches!(event.body, Body::MessageUser(_))
        })
        .count();
    let turns = log
        .iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count();
    (said, turns)
}

#[tokio::test]
async fn a_task_a_person_made_opens_a_turn_and_is_recorded_as_ran() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let hand = Hand::at(START);
    let (core, _client, session) = oneshot(&home, &script, &hand).await;
    core.schedules()
        .add(every_ten_minutes(&session, Creator::Person))
        .unwrap();
    hand.advance(600_000);
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let by = log
        .iter()
        .find_map(|event| match &event.by {
            By::Schedule(by) => Some(by.clone()),
            _ => None,
        })
        .expect("交进来了");
    assert!(by.person, "日志里记着是人建的：{by:?}");
    assert_eq!(
        until_recorded(&core, "t1").await.last.unwrap().result,
        "ran"
    );
}

#[tokio::test]
async fn her_own_task_there_is_only_recorded_and_says_held() {
    let home = Home::new();
    let script = Script::new([Play::Says("不该轮到我。")]);
    let hand = Hand::at(START);
    let (core, _client, session) = oneshot(&home, &script, &hand).await;
    let hers = Creator::Model {
        session: SessionId::parse(&session).unwrap(),
    };
    core.schedules()
        .add(every_ten_minutes(&session, hers))
        .unwrap();
    hand.advance(600_000);
    let last = until_recorded(&core, "t1").await.last.unwrap();
    assert_eq!(last.result, "held", "没开轮的不写 ran");
    assert_eq!(
        last.session.as_ref().map(SessionId::as_str),
        Some(session.as_str())
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        said_and_turns(&home, &session),
        (1, 0),
        "那一句记下了，没开轮"
    );
}

#[tokio::test]
async fn her_own_task_in_a_session_somebody_watches_still_runs() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let hand = Hand::at(START);
    let (core, mut client, session) = oneshot(&home, &script, &hand).await;
    client.subscribe("s1", &session).await;
    let hers = Creator::Model {
        session: SessionId::parse(&session).unwrap(),
    };
    core.schedules()
        .add(every_ten_minutes(&session, hers))
        .unwrap();
    hand.advance(600_000);
    home.until_turns(&session, 1).await;
    assert_eq!(
        until_recorded(&core, "t1").await.last.unwrap().result,
        "ran"
    );
}
