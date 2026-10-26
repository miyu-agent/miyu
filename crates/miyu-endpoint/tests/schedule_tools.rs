//! 她用定时工具（施工 J-2，设计 `45-定时和循环.md` 第三节第 3、4 条）：真核心的会话里她照剧本调 `cron_create`、`cron_list`、
//! `cron_delete`、`schedule_wakeup`，经端口动属主的唤醒表。她建的记成这个会话建的、跑在这个会话上；动态循环那一行她定下一次、
//! 换那句话；没开循环的说没有在跑的循环。

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

/// 属主的表。
fn listed(core: &Core) -> Vec<Schedule> {
    core.schedules().list(&alice()).expect("读得出")
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
    assert_eq!(
        row.when,
        When::Cron {
            cron: "*/5 * * * *".to_string(),
            recurring: true
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
        core.schedules()
            .add(&alice(), mine(session, None))
            .expect("建得了");
    })
    .await;
    assert!(listed(&core).is_empty(), "删了");
}

#[tokio::test]
async fn she_sets_the_next_wakeup_of_this_session_s_loop() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[(
            "schedule_wakeup",
            r#"{"delay_seconds":120,"prompt":"接着盯","reason":"等构建"}"#,
        )]),
        Play::Says("等着。"),
    ];
    let started = std::sync::Mutex::new(None);
    let (core, _) = ran(&home, plays, |core, session| {
        *started.lock().unwrap() = Some(core.schedules().now());
        // 人敲 `/loop` 建的那种：属主建的、跑在这个会话上。
        let looping = Wanted {
            by: Creator::Person { account: alice() },
            ..mine(session, Some(Loop::Dynamic))
        };
        core.schedules().add(&alice(), looping).expect("建得了");
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
async fn without_a_loop_she_hears_there_is_none() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("schedule_wakeup", r#"{"delay_seconds":120}"#)]),
        Play::Says("没有。"),
    ];
    let (core, session) = ran(&home, plays, |_, _| {}).await;
    assert!(listed(&core).is_empty());
    let said = home.log(&session.to_string());
    let result = said
        .iter()
        .find_map(|event| match &event.body {
            miyu_kernel::event::Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("有结果");
    assert!(
        result.contains("No loop is running") || result.contains("no loop"),
        "{result}"
    );
}

#[tokio::test]
async fn she_stops_the_loop_and_cannot_pass_the_cap() {
    let home = Home::new();
    let plays = vec![
        Play::calls(&[("schedule_wakeup", r#"{"stop":true}"#)]),
        Play::calls(&[(
            "cron_create",
            r#"{"cron":"0 9 * * *","prompt":"第二十一个","recurring":true}"#,
        )]),
        Play::Says("好。"),
    ];
    let (core, session) = ran(&home, plays, |core, session| {
        // 循环那一行排在她建的二十个后面（t21）：停的照 `loop` 认，不是照先后。
        for _ in 0..miyu_tool::PER_SESSION {
            core.schedules()
                .add(&alice(), mine(session, None))
                .expect("建得了");
        }
        let looping = Wanted {
            by: Creator::Person { account: alice() },
            ..mine(session, Some(Loop::Dynamic))
        };
        core.schedules().add(&alice(), looping).expect("建得了");
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
