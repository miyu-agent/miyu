//! 人生就是这个会话的循环（施工 LF-13，`docs/blueprint/life.md`「人生会话里的 `/loop`、`/stop`」，`schedules.md` 第三节第 4 条）：
//! 真核心、手拨的钟。人生会话里 `/loop` 不再另开一行循环：光打的回人生的状态，`off` 是暂停人生，带话的不收；`/stop` 打断
//! 这一轮、同时暂停人生；到点时她正在跑一轮的，等这一轮结束再醒。人生那一行给人看得到（`schedule.list`、`schedules` 流），
//! 带上是哪个人格的。
//!
//! 起因是项目主人 2026-11-17 本机验收第 23 条：人生会话里再敲 `/loop` 多出一行循环，她定下一次时定到了人生那一行上，新的
//! 那一行这一轮结束就被删掉，图标亮一下又没了。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Barrier;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::Loop;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::life_command::{GAP, START, asleep, life_row, offset, said, typed};
use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 人生会话里带话的 `/loop` 的回执。
const NOT_HERE: &str =
    "这是人生会话，什么时候继续由 AI 自己定，这里不另开循环；想说什么直接发消息。";

/// 暂停了的那一句。
const PAUSED: &str = "人生会话已暂停，用 /life resume 继续。";

/// 循环这个包的工具，加一件等闸门放行才回的 `look`（放在循环这个包里：人生的预设没有基础系统的工具）。
fn tools(gate: &Arc<Barrier>) -> Catalog {
    let mut looping = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    looping.push(Fake::new("look", Access::Read, Act::Meets(Arc::clone(gate))) as Arc<dyn Tool>);
    Catalog::in_packages([(miyu_schedule::LOOP, looping)]).expect("合写法")
}

/// 只有循环这个包的工具：会话的工具面上有 `schedule_next`，才认 `/loop`。
fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

/// 起好唤醒表的核心、握好手的连接，和人格 `muse` 的一条人生会话：她头一次醒完、睡下了。
async fn living(
    home: &Home,
    script: &Script,
    hand: &Arc<Hand>,
    tools: Catalog,
) -> (Arc<Core>, Client, String) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let core = core_with_tools(home, script, hand, tools);
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let opened = client
        .call("l1", "life.start", json!({"persona": "muse"}))
        .await;
    let session = opened["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("开出来：{opened}"))
        .to_string();
    home.until_turns(&session, 1).await;
    asleep(&mut client, &session).await;
    (core, client, session)
}

/// 跑在会话 `session` 上的循环那几行各是哪一种。
fn loops(core: &Core, session: &str) -> Vec<Loop> {
    core.schedules()
        .list()
        .expect("读得了")
        .into_iter()
        .filter(|one| format!("{:?}", one.target).contains(session))
        .filter_map(|one| one.looping)
        .collect()
}

/// 唤醒表交进来的那几条消息。
fn woken(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.by, By::Schedule(_)))
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .collect()
}

/// 人生会话里的 `/loop`：光打的回人生的状态；带话的、带间隔的不收，回一句；都不多出一行循环，她也没被多叫醒一次。
#[tokio::test]
async fn loop_in_a_life_session_starts_no_second_loop() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, session) = living(&home, &script, &hand, looping()).await;

    let next = Timestamp::from_unix_millis(START + GAP)
        .expect("在范围里")
        .local_minute(offset());
    let reply = typed(&mut client, "k1", &session, "/loop").await;
    assert_eq!(said(&reply), format!("人生会话进行中，下次继续：{next}。"));
    for (id, text) in [
        ("k2", "/loop 看看新闻"),
        ("k3", "/loop 5m 看看新闻"),
        ("k4", "/loop 看看新闻 every 2h"),
    ] {
        let reply = typed(&mut client, id, &session, text).await;
        assert_eq!(said(&reply), NOT_HERE, "{text}");
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(loops(&core, &session), [Loop::Life], "没多出一行");
    assert_eq!(woken(&home.log(&session)).len(), 1, "没被多叫醒");
    let row = life_row(&core, &session).expect("那一行在");
    assert!(!row.paused, "没停");
    assert_eq!(row.prompt, "", "她留给自己的话没被换掉");
}

/// 人生会话里的 `/loop off`（`clear`、`stop` 同）：暂停人生，同 `/life pause`；那一行还在，到点不醒。
#[tokio::test]
async fn loop_off_in_a_life_session_pauses_the_life() {
    for word in ["off", "clear", "STOP"] {
        let home = Home::new();
        let script = Script::new([Play::Says("醒了。")]);
        let hand = Hand::at(START);
        let (core, mut client, session) = living(&home, &script, &hand, looping()).await;
        let reply = typed(&mut client, "k1", &session, &format!("/loop {word}")).await;
        assert_eq!(said(&reply), PAUSED, "{word}");
        assert!(
            life_row(&core, &session).expect("那一行还在").paused,
            "{word}"
        );
        hand.advance(3 * GAP);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(woken(&home.log(&session)).len(), 1, "{word}：到点不醒");
        // 再打一次：已经停着，回同一句。
        let reply = typed(&mut client, "k2", &session, &format!("/loop {word}")).await;
        assert_eq!(said(&reply), PAUSED, "{word}");
    }
}

/// 人生会话里的 `/stop`：打断这一轮，同时暂停人生（全停）；回执写明暂停了、怎么继续。结束不了人生：那一行还在、没封存。
#[tokio::test]
async fn stop_in_a_life_session_interrupts_and_pauses_the_life() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([Play::Says("醒了。"), Play::calls(&[("look", "{}")])]);
    let hand = Hand::at(START);
    let (core, mut client, session) = living(&home, &script, &hand, tools(&gate)).await;
    client.say("s1", &session, "在吗").await;
    until("她停在工具上", || script.requests().len() == 2).await;

    let reply = typed(&mut client, "k1", &session, "/stop").await;
    assert_eq!(
        said(&reply),
        "已全部停止，人生会话已暂停；用 /life resume 继续。"
    );
    home.until_turns(&session, 2).await;
    assert!(
        life_row(&core, &session).expect("那一行还在").paused,
        "停了"
    );
    let log = home.log(&session);
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::LifeEnded(_) | Body::SessionSealed(_))),
        "没结束：{log:?}"
    );
    hand.advance(3 * GAP);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(woken(&home.log(&session)).len(), 1, "到点不醒");
    drop(gate);
}

/// 到点时她正在跑一轮（人在和她说话）：不并进这一轮，等它结束再醒、自己开一轮（同循环的规矩，施工 LO-2）。
#[tokio::test]
async fn a_wake_due_mid_turn_waits_for_the_turn_to_end() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([
        Play::Says("醒了。"),
        Play::calls(&[("look", "{}")]),
        Play::Says("聊完了。"),
        Play::Says("又醒了。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client, session) = living(&home, &script, &hand, tools(&gate)).await;
    client.say("s1", &session, "在吗").await;
    until("她停在工具上", || script.requests().len() == 2).await;
    hand.advance(GAP + 1_000);
    // 给唤醒表一点时间：以前它这时已经交进这一轮了。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(woken(&home.log(&session)).len(), 1, "这一轮里不交");
    gate.wait().await;
    home.until_turns(&session, 3).await;
    let log = home.log(&session);
    let said = woken(&log);
    assert_eq!(said.len(), 2, "{log:?}");
    let second_end = log
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .nth(1)
        .expect("人开的那一轮结束了");
    assert!(said[1].seq > second_end.seq, "那一轮结束以后才醒：{log:?}");
}

/// 人生那一行给人看得到：`schedule.list` 列它，带 `loop: "life"`、下一次、是哪个人格的。
#[tokio::test]
async fn the_life_row_is_listed_with_its_persona() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client, session) = living(&home, &script, &hand, looping()).await;
    let listed = client.call("q1", "schedule.list", json!({})).await;
    let rows: &Vec<Value> = listed["result"]["schedules"].as_array().expect("有表");
    assert_eq!(rows.len(), 1, "{listed}");
    let row = &rows[0];
    assert_eq!(row["loop"], "life", "{row}");
    assert_eq!(row["persona"], "muse", "{row}");
    assert_eq!(row["target"]["session"], session, "{row}");
    assert_eq!(row["paused"], false, "{row}");
    let next = Timestamp::from_unix_millis(START + GAP).expect("在范围里");
    assert_eq!(row["next"], json!(next), "{row}");
}

/// 等到一条 `schedules.changed`，它的那一项带不带 `busy` 是 `busy`；别的推送跳过。
async fn changed(watcher: &mut Client, busy: bool) -> Value {
    loop {
        let message = watcher
            .next_within(Duration::from_secs(60))
            .await
            .expect("等得到推送");
        if message["method"] == "schedules.changed"
            && (message["params"]["entry"]["busy"] == true) == busy
        {
            return message["params"].clone();
        }
    }
}

/// 定时表里人生那一行写得出这一轮在不在跑（施工 LF-15，头的人生卡照它写「这一轮正在跑」）：她停在工具上时列表里那一行带
/// `busy: true`、推了一次带它的；这一轮结束以后不带、又推一次。会话列表那一项不拿推演方法当预览。
#[tokio::test]
async fn the_life_row_says_when_a_round_is_running() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([
        Play::Says("醒了。"),
        Play::calls(&[("look", "{}")]),
        Play::Says("聊完了。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client, session) = living(&home, &script, &hand, tools(&gate)).await;
    let row = |listed: &Value| listed["result"]["schedules"][0].clone();
    let listed = client.call("q1", "schedule.list", json!({})).await;
    assert!(row(&listed).get("busy").is_none(), "她睡下了：{listed}");
    let sessions = client.call("q0", "session.list", json!({})).await;
    let item = sessions["result"]["sessions"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["session"] == json!(session)))
        .cloned()
        .unwrap_or_else(|| panic!("列得出：{sessions}"));
    assert!(item.get("preview").is_none(), "不拿推演方法当预览：{item}");

    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    watcher
        .call("s", "subscribe", json!({"stream": "schedules"}))
        .await;
    client.say("s1", &session, "在吗").await;
    until("她停在工具上", || script.requests().len() == 2).await;
    let on = changed(&mut watcher, true).await;
    assert_eq!(on["entry"]["loop"], "life", "{on}");
    let listed = client.call("q2", "schedule.list", json!({})).await;
    assert_eq!(row(&listed)["busy"], true, "这一轮正在跑：{listed}");

    gate.wait().await;
    home.until_turns(&session, 2).await;
    asleep(&mut client, &session).await;
    let off = changed(&mut watcher, false).await;
    assert!(off["entry"].get("busy").is_none(), "{off}");
    let listed = client.call("q3", "schedule.list", json!({})).await;
    assert!(row(&listed).get("busy").is_none(), "这一轮结束了：{listed}");
}
