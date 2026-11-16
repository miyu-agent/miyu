//! 回合进行中敲的 `/loop`（施工 LO-2）：真核心、手拨的钟。会话正在跑一轮时到点的循环不并进这一轮，等这一轮结束再交、自己开
//! 一轮；`/stop` 连这个会话的循环一起停。以前到点就交进正在跑的这一轮，她下一步顺着手上的活看到了，不定下一次，循环就停了，
//! 那一行停着等她定、头上的图标一直在（项目主人 2026-11-16 报的）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Barrier;

use miyu_endpoint::Core;
use miyu_endpoint::schedules::Schedule;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 循环这个包的工具，加一件等闸门放行才回的 `look`：她这一步停在工具上，回合还要再请求一次。
fn tools(gate: &Arc<Barrier>) -> Catalog {
    let looping = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    let look = Fake::new("look", Access::Read, Act::Meets(Arc::clone(gate))) as Arc<dyn Tool>;
    Catalog::in_packages([
        (miyu_schedule::LOOP, looping),
        (miyu_tool::BASESYSTEM, vec![look]),
    ])
    .expect("合写法")
}

/// 敲一条命令，交回回执。
async fn typed(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

fn rows(core: &Core) -> Vec<Schedule> {
    core.schedules().list().expect("读得出")
}

/// 唤醒表交进来的那几条消息。
fn from_schedule(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.by, By::Schedule(_)))
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .collect()
}

/// 起好的一份：核心、连接、会话；她这一轮先调 `look`（停在闸门上），再说完。
async fn started(home: &Home, script: &Script, gate: &Arc<Barrier>) -> (Arc<Core>, Client, String) {
    let hand = Hand::at(START);
    let core = core_with_tools(home, script, &hand, tools(gate));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    client.say("s1", &session, "查一下部署").await;
    (core, client, session)
}

/// 她这一步停在工具上时敲 `/loop 盯着部署`：这一轮里一个字都不交进来，这一轮结束以后交、由它自己开一轮。
#[tokio::test]
async fn a_loop_typed_mid_turn_runs_in_its_own_turn_after_it() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([
        Play::calls(&[("look", "{}")]),
        Play::Says("部署看完了。"),
        Play::Says("循环看了一眼。"),
    ]);
    let (core, mut client, session) = started(&home, &script, &gate).await;
    until("她停在工具上", || script.requests().len() == 1).await;
    let reply = typed(&mut client, "k1", &session, "/loop 盯着部署").await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    // 给唤醒表一点时间：以前它这时已经交进来了。
    tokio::time::sleep(Duration::from_millis(200)).await;
    gate.wait().await;
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    let said = from_schedule(&log);
    assert_eq!(said.len(), 1, "{log:?}");
    let first_end = log
        .iter()
        .find(|event| matches!(event.body, Body::TurnEnded(_)))
        .expect("第一轮结束了");
    assert!(said[0].seq > first_end.seq, "这一轮结束以后才交：{log:?}");
    let opened = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.trigger),
            _ => None,
        })
        .flatten();
    assert_eq!(opened, Some(said[0].seq), "第二轮由它开");
    let requests = script.requests();
    let second = serde_json::to_string(&requests[1].1.messages).expect("JSON");
    assert!(!second.contains("盯着部署"), "没有并进第一轮：{second}");
    let row = rows(&core).pop().expect("有一行");
    assert_eq!(row.next, None, "交过以后停着等她定");
}

/// 她这一步停在工具上时敲 `/loop`，人再打断（不是 `/stop`）：打断的只是这一轮，循环照样在它结束以后跑第一次。
#[tokio::test]
async fn an_interrupt_still_lets_the_loop_start_after_the_turn() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([Play::calls(&[("look", "{}")]), Play::Says("循环看了一眼。")]);
    let (_core, mut client, session) = started(&home, &script, &gate).await;
    until("她停在工具上", || script.requests().len() == 1).await;
    typed(&mut client, "k1", &session, "/loop 盯着部署").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let params = json!({"session": session, "queued": "keep"});
    let reply = client.call("i1", "session.interrupt", params).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    assert_eq!(from_schedule(&log).len(), 1, "{log:?}");
    let requests = script.requests();
    let last = serde_json::to_string(&requests.last().expect("发了").1.messages).expect("JSON");
    assert!(last.contains("盯着部署"), "{last}");
}

/// 她这一步停在工具上时敲 `/loop`，再 `/stop`：循环那一行没了，这一轮结束以后也不交。
#[tokio::test]
async fn stop_removes_a_loop_typed_mid_turn() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([Play::calls(&[("look", "{}")]), Play::Says("不该有这一轮。")]);
    let (core, mut client, session) = started(&home, &script, &gate).await;
    until("她停在工具上", || script.requests().len() == 1).await;
    typed(&mut client, "k1", &session, "/loop").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let stop = typed(&mut client, "k2", &session, "/stop").await;
    assert_eq!(stop["result"]["said"], "已全部停止。", "{stop}");
    home.until_turns(&session, 1).await;
    assert!(rows(&core).is_empty(), "{:?}", rows(&core));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let log = home.log(&session);
    assert!(from_schedule(&log).is_empty(), "{log:?}");
    assert_eq!(script.requests().len(), 1);
}

/// 循环跑起来以后的一轮里 `/stop`：同样删掉那一行（项目主人看到的：打断、`/stop` 了图标还在）。
#[tokio::test]
async fn stop_in_the_loop_s_own_turn_removes_it() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let script = Script::new([Play::Says("先说完。"), Play::Holds]);
    let (core, mut client, session) = started(&home, &script, &gate).await;
    home.until_turns(&session, 1).await;
    typed(&mut client, "k1", &session, "/loop").await;
    until("循环那一轮请求了", || script.requests().len() == 2).await;
    typed(&mut client, "k2", &session, "/stop").await;
    home.until_turns(&session, 2).await;
    assert!(rows(&core).is_empty(), "{:?}", rows(&core));
}
