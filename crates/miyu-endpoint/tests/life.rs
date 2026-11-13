//! 人生会话（施工 LF-5 中，设计 `70-推演人生.md` 第三节第 1、6、7 条，`docs/blueprint/life.md`）：真核心、手拨的钟。`life.start`
//! 替一个人格开一个会话、记身份、建人生那一行，马上醒第一次；一个人格一条，两个连接同时开也只开出一条；`life.status`、
//! `life.pause`、`life.resume` 照定时表那一行，那一行没了的照日志重建；会话索引那一格照日志派生，删了重建照样认得出来。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Loop, Schedule, Target};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::{By, ScheduleLoop};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_store::index::FILE;
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时（`schedules.md` 第三节第 2 条）。
const GAP: i64 = 3_600_000;

/// 出厂的推演方法，去掉末尾的空白。
fn shipped() -> &'static str {
    include_str!("../../../resources/software/life/method.txt").trim_end()
}

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在时刻的范围里")
}

/// 起好唤醒表的核心、握好手的连接。人格 `muse` 写在数据根里，`engineer` 是出厂包带的。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let core = core_with_tools(home, script, hand, Catalog::default());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client)
}

/// 拒绝的原因码。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"]
        .as_str()
        .unwrap_or_default()
}

/// `life.start` 开出来的会话编号。
async fn start(client: &mut Client, id: &str, persona: &str) -> String {
    let reply = client
        .call(id, "life.start", json!({"persona": persona}))
        .await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string()
}

/// 人生那一行（跑在 `session` 上的）：没有的是空的。
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

/// 日志里人生循环交进来的话的正文。
fn woken(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| {
            let (By::Schedule(by), Body::MessageUser(said)) = (&event.by, &event.body) else {
                return None;
            };
            assert_eq!(by.looping, Some(ScheduleLoop::Life), "{by:?}");
            Some(
                said.blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn start_opens_a_life_session_and_she_wakes_at_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1", "engineer").await;
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第一条是 session.created：{log:?}");
    };
    assert_eq!(created.persona.as_deref(), Some("engineer"));
    assert_eq!(created.preset.as_deref(), Some("life"));
    let capped = log.iter().find_map(|event| match &event.body {
        Body::PolicyChanged(changed) => changed.context.as_ref().and_then(|cap| cap.tokens),
        _ => None,
    });
    assert_eq!(capped, Some(32_000), "上下文上限设成 32000");
    let identity: Vec<&Event> = log
        .iter()
        .filter(|event| matches!(event.body, Body::LifeStarted(_)))
        .collect();
    assert_eq!(identity.len(), 1, "日志里记了身份：{log:?}");
    assert_eq!(identity[0].turn, None);
    assert!(
        matches!(identity[0].by, By::Person(_)),
        "{:?}",
        identity[0].by
    );
    assert_eq!(woken(&log), [shipped()], "第一次醒来是出厂的推演方法");
    let row = life_row(&core, &session).expect("建了人生那一行");
    assert_eq!(row.by, miyu_endpoint::schedules::Creator::Person);
    assert!(!row.paused);
    assert_eq!(row.next, Some(at(START + GAP)), "醒过以后照默认间隔");
}

#[tokio::test]
async fn one_life_per_persona_even_when_two_heads_start_at_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。"), Play::Says("也醒了。")]);
    let hand = Hand::at(START);
    let (core, mut first) = started(&home, &script, &hand).await;
    let mut second = Client::connect(Arc::clone(&core));
    second.hello().await;
    let (a, b) = tokio::join!(
        first.call("a", "life.start", json!({"persona": "muse"})),
        second.call("b", "life.start", json!({"persona": "muse"})),
    );
    let (won, lost) = match a["result"]["session"].as_str() {
        Some(_) => (&a, &b),
        None => (&b, &a),
    };
    let session = won["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("有一个开出来：{a} {b}"));
    assert_eq!(reason(lost), "life_exists", "{lost}");
    assert_eq!(lost["error"]["data"]["session"], json!(session), "{lost}");
    let again = first
        .call("c", "life.start", json!({"persona": "muse"}))
        .await;
    assert_eq!(reason(&again), "life_exists", "{again}");
    assert_eq!(again["error"]["data"]["session"], json!(session));
    let rows: Vec<Schedule> = core
        .schedules()
        .list()
        .expect("读得了")
        .into_iter()
        .filter(|one| one.looping == Some(Loop::Life))
        .collect();
    assert_eq!(rows.len(), 1, "只有一行人生：{rows:?}");
    // 别的人格互不相干。
    let other = start(&mut first, "d", "engineer").await;
    assert_ne!(other, session);
    home.until_turns(session, 1).await;
    home.until_turns(&other, 1).await;
}

#[tokio::test]
async fn status_says_none_running_or_paused() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let none = client
        .call("s1", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(none["result"], json!({"state": "none"}), "{none}");
    let session = start(&mut client, "l1", "muse").await;
    home.until_turns(&session, 1).await;
    let running = client
        .call("s2", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(
        running["result"],
        json!({"state": "running", "session": session, "next": at(START + GAP)}),
        "{running}"
    );
    let paused = client
        .call("p", "life.pause", json!({"persona": "muse"}))
        .await;
    assert_eq!(
        paused["result"],
        json!({"state": "paused", "session": session})
    );
    let status = client
        .call("s3", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(status["result"], paused["result"], "{status}");
}

#[tokio::test]
async fn a_paused_life_sleeps_and_a_resumed_one_wakes_from_now() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        Play::Says("又醒了。"),
        Play::Says("接上了。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1", "muse").await;
    home.until_turns(&session, 1).await;
    let paused = client
        .call("p1", "life.pause", json!({"persona": "muse"}))
        .await;
    assert_eq!(paused["result"]["state"], "paused", "{paused}");
    hand.advance(3 * GAP);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(woken(&home.log(&session)).len(), 1, "停着的到点不醒");
    let resumed = client
        .call("r1", "life.resume", json!({"persona": "muse"}))
        .await;
    assert_eq!(resumed["result"]["state"], "running", "{resumed}");
    assert_eq!(resumed["result"]["session"], json!(session));
    home.until_turns(&session, 2).await;
    let row = life_row(&core, &session).expect("还在");
    assert_eq!(
        row.next,
        Some(at(START + 4 * GAP)),
        "照现在醒、再照默认间隔"
    );

    // 定时表那一行没了（表坏了换过空的）：状态是停着；继续照日志重建，接着醒。
    core.schedules().cancel(&row.id).expect("删得掉");
    let status = client
        .call("s", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(
        status["result"],
        json!({"state": "paused", "session": session})
    );
    let rebuilt = client
        .call("r2", "life.resume", json!({"persona": "muse"}))
        .await;
    assert_eq!(rebuilt["result"]["state"], "running", "{rebuilt}");
    home.until_turns(&session, 3).await;
    let row = life_row(&core, &session).expect("重建了");
    assert_eq!(row.by, miyu_endpoint::schedules::Creator::Person);
    assert_eq!(woken(&home.log(&session)).len(), 3);
}

#[tokio::test]
async fn resuming_a_running_life_changes_nothing_and_no_life_is_refused() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    for method in ["life.pause", "life.resume"] {
        let refused = client.call("x", method, json!({"persona": "muse"})).await;
        assert_eq!(reason(&refused), "no_life", "{method}：{refused}");
    }
    let session = start(&mut client, "l1", "muse").await;
    home.until_turns(&session, 1).await;
    let before = life_row(&core, &session).expect("在");
    let resumed = client
        .call("r", "life.resume", json!({"persona": "muse"}))
        .await;
    assert_eq!(resumed["result"]["state"], "running", "{resumed}");
    assert_eq!(
        life_row(&core, &session).expect("在").next,
        before.next,
        "跑着的不重算"
    );
}

#[tokio::test]
async fn an_unknown_persona_is_refused_by_all_four() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    for method in ["life.start", "life.status", "life.pause", "life.resume"] {
        let refused = client.call("x", method, json!({"persona": "nobody"})).await;
        assert_eq!(reason(&refused), "unknown_persona", "{method}：{refused}");
    }
}

#[tokio::test]
async fn the_list_marks_a_life_session_and_a_rebuilt_index_still_does() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let plain = client.create("c1", "~").await;
    let session = start(&mut client, "l1", "muse").await;
    home.until_turns(&session, 1).await;
    let listed = client.call("l", "session.list", json!({})).await;
    let written = marked(&listed);
    assert_eq!(written, [(session.clone(), true), (plain, false)]);
    // 索引的行都没了（删了重建的空索引）：照日志补回来，那一格一样。
    rusqlite::Connection::open(home.root.index().join(FILE))
        .expect("打得开")
        .execute("DELETE FROM sessions", [])
        .expect("删得了");
    let again = client
        .call("a", "life.start", json!({"persona": "muse"}))
        .await;
    assert_eq!(reason(&again), "life_exists", "照日志认得出来：{again}");
    assert_eq!(again["error"]["data"]["session"], json!(session));
    let listed = client.call("l2", "session.list", json!({})).await;
    assert_eq!(marked(&listed), written, "照日志重建的一样");
}

/// 会话列表每一项：编号、带没带 `life: true`（不是的不写这一格）。
fn marked(listed: &Value) -> Vec<(String, bool)> {
    listed["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .map(|item| {
            let life = match item.get("life") {
                None => false,
                Some(Value::Bool(true)) => true,
                Some(other) => panic!("只写 true：{other}"),
            };
            (
                item["session"].as_str().unwrap_or_default().to_string(),
                life,
            )
        })
        .collect()
}

/// 续上人生那一行照现在算（施工 LF-5 中，`schedules.md` 第六节 `resume`）：它的「什么时候」是某个时刻一次的、早就过了，照它算
/// 会说算不出下一次；她自己定节奏的循环照旧照那一刻算，过了的照旧算不出。
#[tokio::test]
async fn resume_puts_a_life_row_at_now_and_leaves_other_loops_alone() {
    use miyu_endpoint::schedules::{Creator, ScheduleError, Wanted, When};
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, Catalog::default());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let wanted = |looping: Loop| Wanted {
        by: Creator::Person,
        when: When::At { at: at(START) },
        prompt: "过日子".to_string(),
        target: Target::Session {
            session: miyu_kernel::id::SessionId::parse(&session).expect("合写法"),
        },
        looping: Some(looping),
        first: Some(at(START)),
    };
    let life = core.schedules().add(wanted(Loop::Life)).expect("建得了");
    let dynamic = core.schedules().add(wanted(Loop::Dynamic)).expect("建得了");
    for id in [&life.id, &dynamic.id] {
        core.schedules().pause(id).expect("停得了");
    }
    hand.advance(GAP);
    let resumed = core.schedules().resume(&life.id).expect("续得上");
    assert_eq!(resumed.next, Some(at(START + GAP)), "照现在");
    assert!(!resumed.paused);
    assert_eq!(resumed.prompt, "过日子", "她的留言不动");
    assert!(
        matches!(
            core.schedules().resume(&dynamic.id),
            Err(ScheduleError::NoNext)
        ),
        "她自己定节奏的照旧"
    );
}
