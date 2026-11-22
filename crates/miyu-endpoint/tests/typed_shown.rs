//! 发什么显示什么（施工 FX-15，项目主人 2026-11-22 定：「我发什么消息，显示就该是什么消息」）：真核心、假模型。
//!
//! - 人敲 `/loop` 当场开的第一轮，那句 `message.user` 带上人敲的那一行（`command`，同模板命令），之后到点的不带；光打 `/loop`
//!   交进去的是出厂的那句，`by` 写 `builtin`。视图的 `user` 条目多 `command`（命令名），`from` 照抄 `by`。都不进给模型的请求。
//! - 撤回这一轮：那句话跟着撤（同人亲口说的），回执的 `said` 是「/loop …」；循环也删掉。恢复照原样放回那一行；再发一次是换掉。
//! - `/goal <目标>` 当场开的第一轮同样带 `command`，续轮不带。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::Schedule;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::venues::configured_core;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 循环这个包的工具。
fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

/// 敲一条命令，交回回执。
async fn typed(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

/// 照 `by` 挑出来的 `message.user`，照先后，写成 JSON。
fn said_by(log: &[Event], wanted: fn(&By) -> bool) -> Vec<Value> {
    log.iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)) && wanted(&event.by))
        .map(|event| serde_json::to_value(event).expect("写得成"))
        .collect()
}

fn scheduled(by: &By) -> bool {
    matches!(by, By::Schedule(_))
}

fn rows(core: &Core) -> Vec<Schedule> {
    core.schedules().list().expect("读得出")
}

/// 视图里 `user` 条目，照先后。
async fn users(client: &mut Client, id: &str, session: &str) -> Vec<Value> {
    let reply = client
        .call(id, "view.page", json!({"session": session, "view": true}))
        .await;
    reply["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|entry| entry["kind"] == "user")
        .cloned()
        .collect()
}

/// 起好的一份：核心、连接、会话，唤醒表开着。
async fn started(home: &Home, script: &Script) -> (Arc<Core>, Arc<Hand>, Client, String) {
    let hand = Hand::at(START);
    let core = core_with_tools(home, script, &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    (core, hand, client, session)
}

/// 最后一次请求发给模型的消息，写成 JSON 的字。
fn last_request(script: &Script) -> String {
    let requests = script.requests();
    serde_json::to_string(&requests.last().expect("发了").1.messages).expect("JSON")
}

#[tokio::test]
async fn the_first_loop_round_carries_the_typed_line_and_later_ones_do_not() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("又看了。")]);
    let (_core, hand, mut client, session) = started(&home, &script).await;
    typed(&mut client, "k1", &session, "  /loop 5m 看看 CI  ").await;
    home.until_turns(&session, 1).await;
    let first = last_request(&script);
    hand.advance(300_000);
    home.until_turns(&session, 2).await;
    let said = said_by(&home.log(&session), scheduled);
    assert_eq!(said.len(), 2, "{said:?}");
    assert_eq!(
        said[0]["body"]["command"],
        json!({"name": "loop", "text": "/loop 5m 看看 CI"}),
        "第一轮带人敲的那一行（去掉前后空白）：{}",
        said[0]
    );
    assert!(
        said[1]["body"].get("command").is_none(),
        "到点的不带：{}",
        said[1]
    );
    assert!(
        said[0]["by"].get("builtin").is_none(),
        "人写了话的不带 builtin"
    );
    assert_eq!(said[0]["by"]["person"], true);
    assert!(
        !first.contains("/loop"),
        "命令那一行不进给模型的请求：{first}"
    );
    assert!(!first.contains("builtin"), "{first}");

    let shown = users(&mut client, "v1", &session).await;
    assert_eq!(shown[0]["text"], "/loop 5m 看看 CI", "{shown:?}");
    assert_eq!(shown[0]["command"], "loop", "{shown:?}");
    assert_eq!(shown[0]["from"]["kind"], "schedule");
    assert_eq!(shown[1]["text"], "看看 CI", "到点的照原话");
    assert!(shown[1].get("command").is_none(), "{shown:?}");
}

#[tokio::test]
async fn a_bare_loop_says_the_factory_line_marked_builtin() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。"), Play::Says("接着做了。")]);
    let (_core, _hand, mut client, session) = started(&home, &script).await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    typed(&mut client, "k1", &session, "/loop").await;
    home.until_turns(&session, 2).await;
    let said = said_by(&home.log(&session), scheduled);
    assert_eq!(said[0]["by"]["builtin"], true, "{}", said[0]);
    assert_eq!(said[0]["body"]["command"]["text"], "/loop");
    let shown = users(&mut client, "v1", &session).await;
    assert!(
        shown[0].get("command").is_none(),
        "人说的话不带 command：{shown:?}"
    );
    assert_eq!(shown[1]["from"]["builtin"], true, "from 照抄 by：{shown:?}");
    assert_eq!(shown[1]["command"], "loop");
    assert!(!last_request(&script).contains("builtin"));
}

/// 起好的一份：敲 `/loop 5m 看看 CI`、跑了第一轮、撤掉这一轮。交回撤销的回应。
async fn loop_undone(home: &Home, script: &Script) -> (Arc<Core>, Client, String, Value) {
    let (core, _hand, mut client, session) = started(home, script).await;
    typed(&mut client, "k1", &session, "/loop 5m 看看 CI").await;
    home.until_turns(&session, 1).await;
    assert_eq!(rows(&core).len(), 1);
    let reverted = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    (core, client, session, reverted)
}

#[tokio::test]
async fn undoing_the_loop_round_takes_the_line_back_and_stops_the_loop() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("再看一次。")]);
    let (core, mut client, session, reverted) = loop_undone(&home, &script).await;
    assert_eq!(
        reverted["result"]["said"], "/loop 5m 看看 CI",
        "放回输入框的是人敲的那一行：{reverted}"
    );
    assert!(rows(&core).is_empty(), "人撤回了自己敲的 /loop，循环也停");
    let shown = users(&mut client, "v1", &session).await;
    let visible: Vec<&Value> = shown
        .iter()
        .filter(|entry| entry["hidden"] != true)
        .collect();
    assert!(visible.is_empty(), "撤掉的那句不显示：{shown:?}");
    // 人再发一次：换掉，只有一个循环；她只看到一句「看看 CI」。
    typed(&mut client, "k2", &session, "/loop 5m 看看 CI").await;
    home.until_turns(&session, 2).await;
    assert_eq!(rows(&core).len(), 1, "再发一次是换掉，不多开");
    let last = last_request(&script);
    assert_eq!(last.matches("看看 CI").count(), 1, "撤掉的那句不在：{last}");
}

#[tokio::test]
async fn restoring_the_loop_round_puts_the_loop_back_as_it_was() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。")]);
    let (core, mut client, session, reverted) = loop_undone(&home, &script).await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    let before = rows(&core);
    assert!(before.is_empty());
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(restored.get("error").is_none(), "{restored}");
    assert_eq!(restored["result"]["said"], "/loop 5m 看看 CI");
    let back = rows(&core);
    assert_eq!(back.len(), 1, "恢复照原样放回那一行");
    assert_eq!(back[0].id, "t1", "同一个编号");
    assert_eq!(back[0].prompt, "看看 CI");
    assert_eq!(
        back[0].next.map(|next| next.unix_millis()),
        Some(START + 300_000),
        "下一次照原来的"
    );
}

#[tokio::test]
async fn once_the_undo_is_settled_the_loop_does_not_come_back() {
    let home = Home::new();
    let script = Script::new([Play::Says("看过了。"), Play::Says("好。")]);
    let (core, mut client, session, _) = loop_undone(&home, &script).await;
    client.say("s1", &session, "算了").await;
    home.until_turns(&session, 2).await;
    let restored = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert_eq!(reason(&restored), Some("nothing_to_unrevert"), "{restored}");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(rows(&core).is_empty(), "撤回落定了，循环不回来");
}

#[tokio::test]
async fn the_first_goal_round_carries_the_typed_line_and_later_rounds_do_not() {
    let home = Home::new();
    let script = Script::new([Play::Says("在等你确认。"), Play::Says("还在等。")])
        .titles([Play::Says("测试")]);
    let tools = crate::goal_command::goal_tools();
    let mut client = Client::connect(configured_core(&home, &script, tools));
    client.hello().await;
    let session = client.create("c1", "~").await;
    typed(&mut client, "k1", &session, "/goal 把测试修绿").await;
    home.until_turns(&session, 1).await;
    typed(&mut client, "k2", &session, "/goal pause").await;
    typed(&mut client, "k3", &session, "/goal resume").await;
    home.until_turns(&session, 2).await;
    let rounds = said_by(&home.log(&session), |by| matches!(by, By::Module(_)));
    assert_eq!(rounds.len(), 2, "{rounds:?}");
    assert_eq!(
        rounds[0]["body"]["command"],
        json!({"name": "goal", "text": "/goal 把测试修绿"}),
        "{}",
        rounds[0]
    );
    assert!(
        rounds[1]["body"].get("command").is_none(),
        "续轮不带：{}",
        rounds[1]
    );
    let shown = users(&mut client, "v1", &session).await;
    assert_eq!(shown[0]["text"], "/goal 把测试修绿", "{shown:?}");
    assert_eq!(shown[0]["command"], "goal");
    assert!(shown[1].get("command").is_none());
    let first = &script.requests()[0].1.messages;
    assert!(
        !serde_json::to_string(first)
            .unwrap()
            .contains("/goal 把测试修绿"),
        "不进给模型的请求"
    );
}
