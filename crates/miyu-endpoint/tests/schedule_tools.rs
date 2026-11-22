//! 她用定时工具（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：真核心的会话里她照剧本调 `cron_create`、`cron_list`、
//! `cron_delete`、`schedule_next`，经端口动唤醒表。她建的记成这个会话建的、跑在这个会话上；动态循环那一行她定下一次、
//! 换那句话；没开循环的挂一次定时触发（施工 FX-21），没写那句话的说要写。

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

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

/// 真核心，她照 `plays` 回；开一个会话、说一句，等这一轮完。
async fn ran(
    home: &Home,
    plays: Vec<Play>,
    before: impl FnOnce(&Core, &SessionId),
) -> (Arc<Core>, SessionId) {
    let core = Arc::new(home.core_full(&Script::new(plays), catalog(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let id = SessionId::parse(&session).expect("合写法");
    before(&core, &id);
    client.say("s1", &session, "安排一下").await;
    home.until_turns(&session, 1).await;
    (core, id)
}

/// 唤醒表。
fn listed(core: &Core) -> Vec<Schedule> {
    core.schedules().list().expect("读得出")
}

#[tokio::test]
async fn she_creates_lists_and_deletes_on_her_session() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[(
            "cron_create",
            r#"{"cron":"*/5 * * * *","prompt":"看看构建","recurring":true}"#,
        )]),
        Play::calls(&[("cron_list", "{}")]),
        Play::Says("好。"),
    ];
    let (core, session) = ran(&home, plays, |_, _| {}).await;
    let rows = listed(&core);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(
        row.by,
        Creator::Model {
            session: session.clone()
        },
        "记成这个会话建的"
    );
    assert_eq!(
        row.target,
        Target::Session {
            session: session.clone()
        },
        "跑在这个会话上"
    );
    // 时区建时记下这台机器的（施工 J-3 上）。
    assert_eq!(
        row.when,
        When::Cron {
            cron: "*/5 * * * *".to_string(),
            recurring: true,
            zone: jiff::tz::TimeZone::try_system()
                .ok()
                .and_then(|zone| zone.iana_name().map(str::to_string)),
        }
    );
    assert!(row.next.is_some(), "照现在往后算了下一次");

    let home = Home::new();
    let plays = vec![
        Play::calls(&[("cron_delete", r#"{"id":"t1"}"#)]),
        Play::calls(&[("cron_delete", r#"{"id":"t1"}"#)]),
        Play::Says("删了。"),
    ];
    let (core, _) = ran(&home, plays, |core, session| {
        core.schedules().add(mine(session, None)).expect("建得了");
    })
    .await;
    assert!(listed(&core).is_empty(), "删了");
}

#[tokio::test]
async fn she_sets_the_next_wakeup_of_this_session_s_loop() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[(
            "schedule_next",
            r#"{"delay_seconds":120,"prompt":"接着盯","reason":"等构建"}"#,
        )]),
        Play::Says("等着。"),
    ];
    let started = std::sync::Mutex::new(None);
    let (core, _) = ran(&home, plays, |core, session| {
        *started.lock().unwrap() = Some(core.schedules().now());
        // 人敲 `/loop` 建的那种：你建的、跑在这个会话上。
        let looping = Wanted {
            by: Creator::Person,
            ..mine(session, Some(Loop::Dynamic))
        };
        core.schedules().add(looping).expect("建得了");
    })
    .await;
    let rows = listed(&core);
    let row = rows.first().unwrap_or_else(|| panic!("{rows:?}"));
    assert_eq!(row.prompt, "接着盯", "换成她传回来的那句");
    let started = started.lock().unwrap().expect("记了").unix_millis();
    let next = row.next.map(Timestamp::unix_millis).expect("定了下一次");
    assert!(
        (started + 120_000..started + 180_000).contains(&next),
        "下一次在两分钟后：{next} - {started}"
    );
}

#[tokio::test]
async fn a_wakeup_too_far_to_write_waits_until_the_last_moment() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("schedule_next", r#"{"delay_seconds":1000000000000000}"#)]),
        Play::Says("等着。"),
    ];
    let (core, _) = ran(&home, plays, |core, session| {
        let looping = Wanted {
            by: Creator::Person,
            ..mine(session, Some(Loop::Dynamic))
        };
        core.schedules().add(looping).expect("建得了");
    })
    .await;
    let rows = listed(&core);
    let row = rows.first().unwrap_or_else(|| panic!("{rows:?}"));
    assert_eq!(
        row.next,
        Some(Timestamp::LATEST),
        "没有上限，写不下的照最晚的时刻（施工 WK-1）"
    );
}

/// 没开循环、没写那句话（施工 FX-21）：说要写，什么都不建。
#[tokio::test]
async fn without_a_loop_she_must_say_what_to_do() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("schedule_next", r#"{"delay_seconds":120}"#)]),
        Play::Says("没有。"),
    ];
    let (core, session) = ran(&home, plays, |_, _| {}).await;
    assert!(listed(&core).is_empty());
    let said = format!("{:?}", home.log(&session.to_string()));
    assert!(
        said.contains("prompt is required when no loop is running"),
        "{said}"
    );
}

/// 没开循环也能定时（施工 FX-21，项目主人 2026-11-21 定）：挂一次定时触发，记成她在这个会话里建的、某个时刻跑一次、
/// 不是循环；再定换掉原来的，一个会话只挂一个；`stop` 撤掉。
#[tokio::test]
async fn without_a_loop_she_schedules_one_trigger_and_can_replace_and_stop_it() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[(
            "schedule_next",
            r#"{"delay_seconds":120,"prompt":"看看构建好了没有"}"#,
        )]),
        Play::calls(&[(
            "schedule_next",
            r#"{"delay_seconds":600,"prompt":"再看一次构建"}"#,
        )]),
        Play::Says("好。"),
    ];
    let (core, session) = ran(&home, plays, |_, _| {}).await;
    let rows = listed(&core);
    assert_eq!(rows.len(), 1, "再定换掉原来的：{rows:?}");
    let row = &rows[0];
    assert_eq!(row.prompt, "再看一次构建");
    assert!(row.looping.is_none(), "不是循环");
    assert!(matches!(row.when, When::At { .. }), "某个时刻跑一次");
    assert!(matches!(&row.by, Creator::Model { session: by } if *by == session));
    let next = row.next.expect("有下一次").unix_millis();
    let now = core.schedules().now().unix_millis();
    assert!(
        (now + 500_000..=now + 600_000).contains(&next),
        "约十分钟以后：{next} 对 {now}"
    );

    let home = Home::new();
    let plays = vec![
        Play::calls(&[(
            "schedule_next",
            r#"{"delay_seconds":120,"prompt":"看看构建好了没有"}"#,
        )]),
        Play::calls(&[("schedule_next", r#"{"stop":true}"#)]),
        Play::Says("好。"),
    ];
    let (core, _) = ran(&home, plays, |_, _| {}).await;
    assert!(listed(&core).is_empty(), "撤掉了");
}

#[tokio::test]
async fn she_stops_the_loop_and_cannot_pass_the_cap() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("schedule_next", r#"{"stop":true}"#)]),
        Play::calls(&[(
            "cron_create",
            r#"{"cron":"0 9 * * *","prompt":"第二十一个","recurring":true}"#,
        )]),
        Play::Says("好。"),
    ];
    let (core, session) = ran(&home, plays, |core, session| {
        // 循环那一行排在她建的二十个后面（t21）：停的照 `loop` 认，不是照先后。
        for _ in 0..miyu_tool::PER_SESSION {
            core.schedules().add(mine(session, None)).expect("建得了");
        }
        let looping = Wanted {
            by: Creator::Person,
            ..mine(session, Some(Loop::Dynamic))
        };
        core.schedules().add(looping).expect("建得了");
    })
    .await;
    let rows = listed(&core);
    assert!(
        rows.iter().all(|row| row.looping.is_none()),
        "循环那一行删了：{rows:?}"
    );
    assert_eq!(rows.len(), miyu_tool::PER_SESSION, "满了的不再建");
    let said = format!("{:?}", home.log(&session.to_string()));
    assert!(said.contains("cron/full"), "说满了：{said}");
}

/// 跑在这个会话上、她建的一行（`looping` 是 `/loop` 的那种）：一小时以后第一次。
fn mine(session: &SessionId, looping: Option<Loop>) -> Wanted {
    Wanted {
        by: Creator::Model {
            session: session.clone(),
        },
        when: When::Every { every: 3600 },
        prompt: "盯着".to_string(),
        target: Target::Session {
            session: session.clone(),
        },
        looping,
        first: None,
    }
}
