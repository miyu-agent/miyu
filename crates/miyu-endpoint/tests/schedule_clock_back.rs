//! 系统时间往回拨以后同一个时刻又到点（施工 FX-2，`docs/blueprint/schedules.md` 第三节第 4 条）：交进会话的命令编号每一次都是
//! 新的，内核不把第二次当成重发。以前编号是 `schedule/<任务>/<到点的毫秒>/say`：cron 照现在算下一次，钟拨回去以后人停了再续，
//! 下一次落在拨回前跑过的那一刻，编号和那一次一样，内核照上一次回应、不开轮，这一次白白丢掉。

use std::sync::Arc;
use std::time::Duration;

use miyu_endpoint::schedules::{Creator, Target, Wanted, When};
use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::clock::{Hand, core_with};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;
/// 十分钟。
const TEN_MINUTES: i64 = 600_000;

#[tokio::test]
async fn the_same_moment_due_again_after_the_clock_went_back_still_speaks() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("又看了一遍。")]);
    let hand = Hand::at(START);
    let core = core_with(&home, &script, &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &cwd).await;
    core.start_schedules();
    let schedules = core.schedules();
    let added = schedules
        .add(Wanted {
            by: Creator::Person,
            when: When::Cron {
                cron: "*/10 * * * *".to_string(),
                recurring: true,
                zone: Some("UTC".to_string()),
            },
            prompt: "看看 CI".to_string(),
            target: Target::Session {
                session: SessionId::parse(&session).expect("会话编号合写法"),
            },
            looping: None,
            first: None,
        })
        .unwrap();
    let due = Timestamp::from_unix_millis(START + TEN_MINUTES);
    assert_eq!(added.next, due);
    hand.advance(TEN_MINUTES);
    home.until_turns(&session, 1).await;
    // 等这一次记完上一次：后面的停、续不和它抢着写这一行。
    let recorded = tokio::time::timeout(Duration::from_secs(60), async {
        while schedules.get(&added.id).unwrap().last.is_none() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(recorded.is_ok(), "六十秒内记了上一次");

    // 系统时间往回拨了十分钟，人停了再续：照现在算，下一次又是刚跑过的那一刻。
    hand.advance(-TEN_MINUTES);
    schedules.pause(&added.id).unwrap();
    let resumed = schedules.resume(&added.id).unwrap();
    assert_eq!(resumed.next, due, "同一个时刻又到点");
    hand.advance(TEN_MINUTES);
    home.until_turns(&session, 2).await;

    let log = home.log(&session);
    let said: Vec<_> = log
        .iter()
        .filter(|event| {
            matches!(event.by, By::Schedule(_)) && matches!(event.body, Body::MessageUser(_))
        })
        .filter_map(|event| event.cause.clone())
        .collect();
    assert_eq!(said.len(), 2, "两次都交进去了：{said:?}");
    assert_ne!(said[0], said[1], "两次的命令编号不一样");
}
