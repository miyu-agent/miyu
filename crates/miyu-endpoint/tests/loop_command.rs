//! `/loop`（施工 J-2，设计 `45-定时和循环.md` 第三节第 4 条）：真核心、手拨的钟。写了间隔的建一行固定循环、马上交进来跑第一次，
//! 之后照间隔；没写的建一行动态循环、马上跑一次，交过以后停着等她定；再敲一次换掉原来的；`/loop off` 删掉；工具面上没有
//! `schedule_wakeup` 的会话里没有这条命令。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, When};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 循环这个包的工具。
fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

/// 起好唤醒表的核心、开好的会话，敲一条命令，交回回执。每次一个新的请求编号：同一个编号换了一句是 `id_reused`（施工 YC-7）。
async fn typed(client: &mut Client, session: &str, text: &str) -> Value {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = format!(
        "k{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    client
        .call(
            &id,
            "command.run",
            json!({"session": session, "text": text}),
        )
        .await
}

fn rows(core: &Core) -> Vec<Schedule> {
    core.schedules().list().expect("读得出")
}

#[tokio::test]
async fn a_fixed_loop_runs_now_and_then_every_interval() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("又看了。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let reply = typed(&mut client, &session, "/loop 5m 看看 CI").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：每 5m · 看看 CI",
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    let row = rows(&core).pop().expect("有一行");
    assert_eq!(row.looping, Some(Loop::Fixed));
    assert_eq!(row.when, When::Every { every: 300 });
    assert_eq!(row.by, Creator::Person);
    assert!(matches!(&row.target, Target::Session { session: on } if on.to_string() == session));
    assert_eq!(
        row.next.map(|next| next.unix_millis()),
        Some(START + 300_000),
        "跑过第一次，下一次照间隔"
    );
    hand.advance(300_000);
    home.until_turns(&session, 2).await;
}

#[tokio::test]
async fn a_self_paced_loop_runs_once_then_waits_and_off_removes_it() {
    let home = Home::new();
    // 换成固定循环的那一次也马上跑：剧本多排一句。
    let script = Script::new([Play::Says("先看一眼。"), Play::Says("换了。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let reply = typed(&mut client, &session, "/loop 盯着部署").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：由 AI 定下一次 · 盯着部署",
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    let row = rows(&core).pop().expect("有一行");
    assert_eq!(row.looping, Some(Loop::Dynamic));
    assert_eq!(row.next, None, "交过以后停着等她定");
    // 再敲一次换掉原来的：同一个会话只有一个循环。
    typed(&mut client, &session, "/loop 10m 换一句").await;
    home.until_turns(&session, 2).await;
    let now = rows(&core);
    assert_eq!(now.len(), 1, "{now:?}");
    assert_eq!(now[0].prompt, "换一句");
    let off = typed(&mut client, &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "已停止此会话的循环。", "{off}");
    assert!(rows(&core).is_empty());
    let again = typed(&mut client, &session, "/loop off").await;
    assert_eq!(again["result"]["said"], "此会话没有循环。", "{again}");
}

/// `/loop off` 只删循环：同一个会话上的定时任务照旧留着。
#[tokio::test]
async fn off_leaves_the_session_s_other_schedules() {
    let home = Home::new();
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &Script::new([]), &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let plain = miyu_endpoint::schedules::Wanted {
        by: Creator::Person,
        when: When::Every { every: 3600 },
        prompt: "每小时看一次".to_string(),
        target: Target::Session {
            session: miyu_kernel::id::SessionId::parse(&session).expect("合写法"),
        },
        looping: None,
        first: None,
    };
    core.schedules().add(plain).expect("建得了");
    let off = typed(&mut client, &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "此会话没有循环。", "{off}");
    assert_eq!(rows(&core).len(), 1, "定时任务留着");
}

#[tokio::test]
async fn without_the_loop_package_there_is_no_such_command() {
    let home = Home::new();
    let hand = Hand::at(START);
    let core = core_with(&home, &Script::new([]), &hand);
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = typed(&mut client, &session, "/loop 5m 看看").await;
    assert_eq!(
        reply["error"]["data"]["reason"], "unknown_command",
        "{reply}"
    );
    assert!(rows(&core).is_empty());
}

/// 敲一条命令，请求编号照给的写。
async fn typed_as(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

/// 同一个编号原样重发（施工 YC-7补）：不再跑第二遍，回应和头一次一样。动态循环交过第一次以后停着，再跑一遍的话会被
/// 换成一行新的、又马上交一次。
#[tokio::test]
async fn a_resent_loop_command_does_not_run_twice() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("不该有第二轮。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let first = typed_as(&mut client, "again-1", &session, "/loop 盯着部署").await;
    assert!(first["result"]["events"].is_array(), "{first}");
    home.until_turns(&session, 1).await;
    let before = rows(&core);
    assert_eq!(before.len(), 1, "{before:?}");
    assert_eq!(before[0].next, None, "交过第一次，停着等她定");
    let again = typed_as(&mut client, "again-1", &session, "/loop 盯着部署").await;
    assert_eq!(again["result"], first["result"], "{again}");
    let after = rows(&core);
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!(after[0].id, before[0].id, "还是原来那一行");
    assert_eq!(after[0].next, None, "没有再交一次");
    // 新的编号照旧：换掉原来的。
    let fresh = typed_as(&mut client, "again-2", &session, "/loop 盯着部署").await;
    assert!(fresh["result"]["events"].is_array(), "{fresh}");
    home.until_turns(&session, 2).await;
    let replaced = rows(&core);
    assert_eq!(replaced.len(), 1, "{replaced:?}");
    assert_ne!(replaced[0].id, before[0].id, "新编号照旧执行");
}

/// 同一个编号换了一句重发（施工 YC-7补）：`id_reused`，命令没有跑，循环还在。
#[tokio::test]
async fn a_resent_id_with_another_command_is_refused_before_it_runs() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let first = typed_as(&mut client, "reuse-1", &session, "/loop 5m x").await;
    assert!(first["result"]["events"].is_array(), "{first}");
    home.until_turns(&session, 1).await;
    let off = typed_as(&mut client, "reuse-1", &session, "/loop off").await;
    assert_eq!(reason(&off), Some("id_reused"), "{off}");
    let now = rows(&core);
    assert_eq!(now.len(), 1, "循环还在：{now:?}");
    assert_eq!(now[0].prompt, "x");
    // 被拒的不记，新的编号照旧执行。
    let off = typed_as(&mut client, "reuse-2", &session, "/loop off").await;
    assert_eq!(off["result"]["said"], "已停止此会话的循环。", "{off}");
    assert!(rows(&core).is_empty());
}

/// 出厂的自主循环那句（施工 LO-1）：去掉行尾的换行。
fn autonomous() -> &'static str {
    include_str!("../../../resources/software/loop/autonomous.txt").trim_end()
}

/// 光打 `/loop`（施工 LO-1）：同动态的，马上交进一轮，交进去的是出厂的那句，存在那一行的 `prompt` 里。
#[tokio::test]
async fn a_bare_loop_runs_now_on_the_fixed_sentence() {
    let home = Home::new();
    let script = Script::new([Play::Says("我自己看着办。")]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    let reply = typed(&mut client, &session, "/loop").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：由 AI 自己定做什么、下一次什么时候",
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    let row = rows(&core).pop().expect("有一行");
    assert_eq!(row.looping, Some(Loop::Dynamic));
    assert_eq!(row.by, Creator::Person);
    assert_eq!(row.prompt, autonomous());
    assert_eq!(row.next, None, "交过以后停着等她定");
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    let sent = serde_json::to_string(&request.messages.last()).expect("写得成 JSON");
    assert!(sent.contains(autonomous()), "交进去的是那句：{sent}");
}

/// 光打 `/loop` 的第二轮（施工 LO-1）：她定了下一次、到点又交一次，第二轮的请求前缀和第一轮逐字节相同。
#[tokio::test]
async fn a_bare_loop_s_second_wakeup_keeps_the_prefix() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("schedule_wakeup", r#"{"delay_seconds":120}"#)]),
        Play::Says("过两分钟再看。"),
        Play::Says("又看了一眼。"),
    ]);
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    typed(&mut client, &session, "/loop").await;
    home.until_turns(&session, 1).await;
    let row = rows(&core).pop().expect("有一行");
    assert_eq!(row.prompt, autonomous(), "没传 prompt 的照旧");
    hand.advance(120_000);
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let first = &requests.first().expect("第一轮").1;
    let second = &requests.last().expect("第二轮").1;
    assert_eq!(requests.len(), 3, "一轮两次、第二轮一次");
    assert_eq!(bytes(&second.tools), bytes(&first.tools), "工具面不变");
    assert_eq!(second.system, first.system, "system 不变");
    let before = first.messages.len();
    assert!(second.messages.len() > before);
    assert_eq!(
        bytes(&second.messages[..before]),
        bytes(&first.messages),
        "第一轮的消息原样在前"
    );
}

/// 参数写错了（施工 LO-1）：只写了间隔没写话，`bad_params`，`data.why` 带上用法，什么都不建。
#[tokio::test]
async fn a_wrong_loop_answers_with_the_usage() {
    let home = Home::new();
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &Script::new([]), &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = typed(&mut client, &session, "/loop every 5m").await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["why"],
        "参数不对。用法：\n\
         /loop　不给具体要求，由 AI 自己定做什么、什么时候\n\
         /loop <间隔> <要说的话>　每隔一段时间说一次，例如 5m、2h\n\
         /loop <要说的话>　由 AI 自己定下一次什么时候\n\
         /loop off　停止此会话的循环",
        "{reply}"
    );
    assert!(rows(&core).is_empty());
}

/// 请求的几样写成 JSON 比字节。
fn bytes<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).expect("写得成 JSON")
}
