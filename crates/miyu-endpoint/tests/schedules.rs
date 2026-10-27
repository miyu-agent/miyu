//! 唤醒表（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 1、2 条）：真核心、手拨的钟。到点把那句话交进会话，记成
//! `by: schedule`；每次新开的照建时定下的开一个；核心停着时错过的跳过；会话没了的停掉；她建的照会话现在的预设判；她自己定
//! 节奏的循环交过以后停着等她定。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, ScheduleLoop};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

/// 人建的、跑在 `session` 上的一个。
fn on(session: &str, when: When) -> Wanted {
    Wanted {
        by: Creator::Person { account: alice() },
        when,
        prompt: "看看 CI".to_string(),
        target: Target::Session {
            session: SessionId::parse(session).expect("会话编号合写法"),
        },
        looping: None,
        first: None,
    }
}

/// 起好唤醒表的核心、握好手的连接、开好的一个会话。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client, String) {
    let core = core_with(home, script, hand);
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
            let schedule = core.schedules().get(&alice(), id).expect("有这一个");
            if schedule.last.is_some() {
                return schedule;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    waited.unwrap_or_else(|_| panic!("{id} 没跑"))
}

/// 日志里定时交进来的话：`by` 和正文。
fn scheduled(log: &[Event]) -> Vec<(By, String)> {
    log.iter()
        .filter(|event| matches!(event.by, By::Schedule(_)))
        .filter_map(|event| match &event.body {
            Body::MessageUser(message) => Some((event.by.clone(), format!("{:?}", message.blocks))),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_due_schedule_speaks_into_its_session_and_steps_on() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let hand = Hand::at(START);
    let (core, _client, session) = started(&home, &script, &hand).await;
    let added = core
        .schedules()
        .add(&alice(), on(&session, When::Every { every: 600 }))
        .unwrap();
    assert_eq!(added.id, "t1");
    assert_eq!(added.next, Some(at(START + 600_000)));
    hand.advance(599_000);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(scheduled(&home.log(&session)).is_empty(), "没到点不交");
    hand.advance(1_000);
    home.until_turns(&session, 1).await;
    let said = scheduled(&home.log(&session));
    assert_eq!(said.len(), 1, "{said:?}");
    let By::Schedule(by) = &said[0].0 else {
        unreachable!()
    };
    assert_eq!(
        (by.schedule.as_str(), &by.account, by.looping),
        ("t1", &alice(), None)
    );
    assert!(said[0].1.contains("看看 CI"), "原话原样交：{}", said[0].1);
    let after = until_recorded(&core, "t1").await;
    let last = after.last.unwrap();
    assert_eq!(last.result, "ran");
    assert_eq!(
        last.session.as_ref().map(SessionId::as_str),
        Some(session.as_str())
    );
    assert_eq!(
        after.next,
        Some(at(START + 1_200_000)),
        "照上一次往后一个间隔"
    );
}

#[tokio::test]
async fn a_new_session_each_time_is_opened_as_chosen_and_a_one_off_goes() {
    let home = Home::new();
    let script = Script::new([Play::Says("早。")]);
    let hand = Hand::at(START);
    let (core, mut client, _) = started(&home, &script, &hand).await;
    let cwd = home.work.to_string_lossy().into_owned();
    core.schedules()
        .add(
            &alice(),
            Wanted {
                target: Target::New {
                    persona: None,
                    preset: "none".to_string(),
                    cwd,
                },
                ..on(
                    "01a17000-0000-7000-8000-000000000001",
                    When::At {
                        at: at(START + 30_000),
                    },
                )
            },
        )
        .unwrap();
    hand.advance(30_000);
    let waited = tokio::time::timeout(Duration::from_secs(5), async {
        while !core.schedules().list(&alice()).unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "一次的跑完删掉");
    let listed = client.call("l", "session.list", json!({})).await;
    let sessions = listed["result"]["sessions"].as_array().unwrap().clone();
    let opened = sessions
        .iter()
        .find(|one| one["preset"] == "none")
        .unwrap_or_else(|| panic!("开了一个照建时预设的会话：{listed}"));
    let id = opened["session"].as_str().unwrap();
    home.until_turns(id, 1).await;
    assert_eq!(scheduled(&home.log(id)).len(), 1);
}

#[tokio::test]
async fn missed_ones_are_skipped_when_the_core_comes_back() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let session = {
        let core = core_with(&home, &script, &hand);
        let mut client = Client::connect(Arc::clone(&core));
        client.hello().await;
        let cwd = home.work.to_string_lossy().into_owned();
        let session = client.create("c1", &cwd).await;
        core.schedules()
            .add(&alice(), on(&session, When::Every { every: 600 }))
            .unwrap();
        core.schedules()
            .add(
                &alice(),
                on(
                    &session,
                    When::At {
                        at: at(START + 60_000),
                    },
                ),
            )
            .unwrap();
        session
    };
    hand.advance(3_600_000 + 1);
    let core = core_with(&home, &script, &hand);
    core.start_schedules();
    let left = core.schedules().list(&alice()).unwrap();
    assert_eq!(left.len(), 1, "一次的错过了就删：{left:?}");
    assert_eq!(left[0].id, "t1");
    assert_eq!(
        left[0].next,
        Some(at(START + 4_200_000)),
        "照现在往后找，错过的不补"
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(scheduled(&home.log(&session)).is_empty(), "没交");
}

#[tokio::test]
async fn a_gone_session_stops_its_schedule() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    core.schedules()
        .add(&alice(), on(&session, When::Every { every: 60 }))
        .unwrap();
    let deleted = client
        .call("d", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    hand.advance(60_000);
    let after = until_recorded(&core, "t1").await;
    assert_eq!(after.last.unwrap().result, "skipped:session_gone");
    assert!(after.paused && after.next.is_none(), "停掉，留给人删");
}

#[tokio::test]
async fn her_own_ones_follow_the_session_s_preset_and_people_s_do_not() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let hand = Hand::at(START);
    // 施工 J-2 起有了 `schedule` 包、在基础系统组里：会话用的预设单独关掉它，她建的不跑。
    home.write(
        "home/alice/presets/quiet.toml",
        "[preset]\nname = { en = \"Quiet\" }\nunlisted = \"off\"\n\n[software]\nbasesystem = true\nschedule = false\n",
    );
    let (core, mut client, _) = started(&home, &script, &hand).await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with("c2", json!({"cwd": cwd, "preset": "quiet"}))
        .await;
    let id = SessionId::parse(&session).unwrap();
    core.schedules()
        .add(
            &alice(),
            Wanted {
                by: Creator::Model { session: id },
                ..on(&session, When::Every { every: 60 })
            },
        )
        .unwrap();
    core.schedules()
        .add(&alice(), on(&session, When::Every { every: 120 }))
        .unwrap();
    hand.advance(60_000);
    let hers = until_recorded(&core, "t1").await;
    assert_eq!(hers.last.unwrap().result, "skipped:preset_off");
    assert_eq!(hers.next, Some(at(START + 120_000)), "下一次照算");
    hand.advance(60_000);
    home.until_turns(&session, 1).await;
    let said = scheduled(&home.log(&session));
    assert_eq!(said.len(), 1, "人建的照跑：{said:?}");
    let By::Schedule(by) = &said[0].0 else {
        unreachable!()
    };
    assert_eq!(by.schedule, "t2");
}

#[tokio::test]
async fn a_dynamic_loop_waits_after_it_speaks_until_she_sets_the_next() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let hand = Hand::at(START);
    let (core, _client, session) = started(&home, &script, &hand).await;
    let added = core
        .schedules()
        .add(
            &alice(),
            Wanted {
                looping: Some(Loop::Dynamic),
                first: Some(at(START)),
                ..on(&session, When::At { at: at(START) })
            },
        )
        .unwrap();
    home.until_turns(&session, 1).await;
    let after = until_recorded(&core, &added.id).await;
    assert_eq!(after.next, None, "交过以后停着等她定");
    assert_eq!(core.schedules().list(&alice()).unwrap().len(), 1, "不删");
    let said = scheduled(&home.log(&session));
    let By::Schedule(by) = &said[0].0 else {
        unreachable!()
    };
    assert_eq!(by.looping, Some(ScheduleLoop::Dynamic));
    core.schedules()
        .set_next(
            &alice(),
            &added.id,
            Some(at(START + 300_000)),
            Some("再看看".to_string()),
        )
        .unwrap();
    hand.advance(300_000);
    home.until_turns(&session, 2).await;
    let said = scheduled(&home.log(&session));
    assert_eq!(said.len(), 2);
    assert!(said[1].1.contains("再看看"), "换过的那句：{}", said[1].1);
}

#[tokio::test]
async fn bad_ones_are_refused_and_pause_resume_cancel_work() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let (core, _client, session) = started(&home, &script, &hand).await;
    let schedules = core.schedules();
    for when in [
        When::Cron {
            cron: "0 0 8 * * *".to_string(),
            recurring: true,
            zone: None,
        },
        When::Every { every: 59 },
        When::At { at: at(START) },
    ] {
        assert!(
            schedules.add(&alice(), on(&session, when.clone())).is_err(),
            "{when:?}"
        );
    }
    assert!(schedules.list(&alice()).unwrap().is_empty(), "什么都没建");
    let added = schedules
        .add(&alice(), on(&session, When::Every { every: 600 }))
        .unwrap();
    let paused = schedules.pause(&alice(), &added.id).unwrap();
    assert!(paused.paused && paused.next.is_none());
    hand.advance(1_000_000);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(scheduled(&home.log(&session)).is_empty(), "停着的不跑");
    let resumed = schedules.resume(&alice(), &added.id).unwrap();
    assert_eq!(resumed.next, Some(at(START + 1_600_000)), "照现在重算");
    schedules.cancel(&alice(), &added.id).unwrap();
    assert!(schedules.get(&alice(), &added.id).is_err());
    assert!(schedules.cancel(&alice(), &added.id).is_err(), "没有的报错");
    let bob = miyu_kernel::id::AccountId::parse("bob").unwrap();
    assert!(schedules.list(&bob).unwrap().is_empty(), "没有表的是空的");
}
