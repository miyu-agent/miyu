//! 她自己结束这段人生（施工 LF-4 下，设计 `70-推演人生.md` 第二节第 3 条、第三节第 5 条，`docs/blueprint/life.md`「结束」）：
//! 真核心、手拨的钟。`end_life` 只在没结束的人生会话的工具面上，从第一轮起每一轮都一样（重起核心照日志载入以后也一样）；拿同一个
//! 预设开的普通会话、别的预设开了 `base-life` 的普通会话都没有，也不算进没开的。她调了：回一句、这一轮照常说完，`life.ended` 的
//! `by` 是那次调用、遗言原样，之后和主人结束一样清理；没写遗言的拒、什么都不记；同一轮里再调回「已经结束了」、不重复记。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Loop, Schedule, Target};
use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_kernel::request::Request;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::preset_face::{snapshot, tool_names};
use crate::support::clock::{Hand, core_with_tools};
use crate::support::mcp::results;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时。
const GAP: i64 = 3_600_000;

/// 成了的那一句。
const ENDED: &str = "You have died. This was your last moment awake.";

/// 记忆、循环、结束人生三个包的真工具。
fn catalog() -> Catalog {
    let resources = default_resources();
    Catalog::in_packages([
        (
            "base-memory",
            miyu_memory::tools(&resources).expect("读得出"),
        ),
        (
            miyu_schedule::LOOP,
            miyu_schedule::loop_tools(&resources).expect("出厂的字"),
        ),
        (
            miyu_schedule::LIFE,
            miyu_schedule::life_tools(&resources).expect("出厂的字"),
        ),
    ])
    .expect("合写法")
}

/// 起好唤醒表的核心、握好手的连接。人格 `muse` 写在数据根里。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let core = core_with_tools(home, script, hand, catalog());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client)
}

async fn start(client: &mut Client, id: &str) -> String {
    let reply = client
        .call(id, "life.start", json!({"persona": "muse"}))
        .await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string()
}

/// 带工具面的第 `n` 次请求（从 0 数）：主请求，起标题这类辅助请求不带工具。每个测试里会话一个接一个地跑。
fn request(script: &Script, n: usize) -> Request {
    script
        .requests()
        .into_iter()
        .map(|(_, request)| request)
        .filter(|request| !request.tools.is_empty())
        .nth(n)
        .unwrap_or_else(|| panic!("发了第 {n} 次请求"))
}

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

fn ended(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.body, Body::LifeEnded(_)))
        .collect()
}

/// `end_life` 的回话，照日志的先后。
fn answers(home: &Home, session: &str) -> Vec<(String, bool)> {
    results(home, session)
        .into_iter()
        .filter(|(name, ..)| name == "end_life")
        .map(|(_, text, error, _)| (text, error))
        .collect()
}

/// 日志里她调 `end_life` 的那几次调用的编号。
fn end_calls(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(message) => Some(message.blocks.clone()),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::ToolCall(call) if call.name == "end_life" => Some(call.call_id.to_string()),
            _ => None,
        })
        .collect()
}

/// 读到一条 `notice` 为止，等 `wait`；等不到的没有。
async fn notice(client: &mut Client, wait: Duration) -> Option<Value> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client.next_within(left).await?;
        if message["method"] == "notice" {
            return Some(message["params"].clone());
        }
    }
}

fn end_life(words: &str) -> Play {
    Play::calls(&[("end_life", &json!({"last_words": words}).to_string())])
}

/// 验收第 1 条：人生会话第一轮就有 `end_life`，第二、三轮逐项一样；改了预设、重起核心照日志载入以后，照新预设重拼的工具面里
/// 照样有它（回合开始照日志问是不是人生会话）。
#[tokio::test]
async fn a_life_session_has_end_life_from_its_first_turn_on() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("醒了。"),
        Play::Says("嗯。"),
        Play::Says("在。"),
        Play::Says("还在。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let first = request(&script, 0);
    let names = |request: &Request| -> Vec<String> {
        tool_names(request)
            .into_iter()
            .map(str::to_string)
            .collect()
    };
    assert!(
        names(&first).contains(&"end_life".to_string()),
        "{:?}",
        names(&first)
    );
    client.say("s1", &session, "今天怎么样").await;
    home.until_turns(&session, 2).await;
    client.say("s2", &session, "还在吗").await;
    home.until_turns(&session, 3).await;
    assert_eq!(request(&script, 1).tools, first.tools, "第二轮一样");
    assert_eq!(request(&script, 2).tools, first.tools, "第三轮一样");
    let pin = snapshot(&home, &session).preset.expect("记了预设");
    assert!(!pin.off.contains(&"life".to_string()), "{:?}", pin.off);

    // 盖一层预设（单关 forget），重起核心：载入以后第一轮照新预设重拼工具面。
    home.write("root/presets/life.toml", "[tools]\nforget = false\n");
    drop(client);
    drop(core);
    let core = core_with_tools(&home, &script, &hand, catalog());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    client.say("s3", &session, "又是一天").await;
    home.until_turns(&session, 4).await;
    let mut wanted = names(&first);
    wanted.retain(|name| name != "forget");
    assert_eq!(
        names(&request(&script, 3)),
        wanted,
        "照新预设重拼，end_life 还在"
    );
}

/// 验收第 1 条：拿 `life` 预设开的普通会话、别的预设开了 `base-life` 的普通会话都没有，也不算进没开的。
#[tokio::test]
async fn other_sessions_never_get_end_life_even_with_its_package_on() {
    let home = Home::new();
    home.write(
        "root/presets/mine.toml",
        "[preset]\nname = { en = \"Mine\" }\nunlisted = \"off\"\n\n[software]\nbase-life = true\nbase-memory = true\n",
    );
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    for (n, preset) in ["life", "mine"].into_iter().enumerate() {
        let session = client
            .create_with(
                &format!("c{n}"),
                json!({"cwd": "~", "persona": "muse", "preset": preset}),
            )
            .await;
        client.say(&format!("s{n}"), &session, "hi").await;
        home.until_turns(&session, 1).await;
        let names = tool_names(&request(&script, n))
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert!(
            !names.contains(&"end_life".to_string()),
            "{preset}：{names:?}"
        );
        assert!(!names.is_empty(), "{preset}：别的工具照给");
        let pin = snapshot(&home, &session).preset.expect("记了预设");
        assert!(
            !pin.off.contains(&"life".to_string()),
            "{preset}：不算进没开的：{:?}",
            pin.off
        );
    }
}

/// 验收第 2 条：她调了，回那一句，这一轮照常说完；`life.ended` 的 `by` 是那次调用、遗言原样；之后封存、那一行没了、不再醒，
/// 这段人生记下的清掉，主人收到 `by: self`、带遗言的通知。
#[tokio::test]
async fn she_ends_her_life_and_it_is_cleared_like_the_owner_did() {
    let home = Home::new();
    let words = "  谢谢你陪我走到这里。  ";
    let script = Script::new([
        Play::calls(&[(
            "remember",
            &json!({"class": "user", "text": "她今天去了海边"}).to_string(),
        )]),
        end_life(words),
        Play::Says("我走了。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    assert_eq!(answers(&home, &session), [(ENDED.to_string(), false)]);
    let told = notice(&mut client, Duration::from_secs(30))
        .await
        .expect("推了通知");
    assert_eq!(told["kind"], "life.ended", "{told}");
    assert_eq!(told["by"], "self");
    assert_eq!(told["last_words"], words, "遗言原样");
    assert_eq!(told["session"], json!(session));
    assert_eq!(told["persona"], "muse");

    let log = home.log(&session);
    let ends = ended(&log);
    assert_eq!(ends.len(), 1, "{log:?}");
    let calls = end_calls(&log);
    assert!(
        matches!(&ends[0].by, By::Tool(tool) if [tool.call_id.to_string()] == *calls),
        "by 是那次调用：{:?} {calls:?}",
        ends[0].by
    );
    assert!(
        matches!(&ends[0].body, Body::LifeEnded(body) if body.last_words.as_deref() == Some(words))
    );
    let said = log.iter().any(|event| {
        matches!(&event.body, Body::MessageAssistant(message)
            if message.blocks.iter().any(|block| matches!(block, Block::Text(text) if text.text == "我走了。")))
    });
    assert!(said, "这一轮照常说完");
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::SessionSealed(_))),
        "封存了"
    );
    assert!(life_row(&core, &session).is_none(), "人生那一行删了");
    let listed = client
        .call("m", "memory.list", json!({"persona": "muse"}))
        .await;
    assert_eq!(listed["result"]["memories"], json!([]), "{listed}");
    hand.advance(5 * GAP);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let turns = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count();
    assert_eq!(turns, 1, "往后拨多久都不再醒");
    let status = client
        .call("st", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(
        status["result"],
        json!({"state": "ended", "session": session})
    );
}

/// 验收第 3、4 条：没写、只有空白的拒，什么都不记；同一轮里先后调两次、一步里一起调两次：第一次记下，别的回「已经结束了」，
/// 只记一条、只推一次。
#[tokio::test]
async fn no_last_words_is_refused_and_a_second_end_redoes_nothing() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("end_life", "{}")]),
        end_life(" \n "),
        Play::calls(&[
            ("end_life", r#"{"last_words":"再见。"}"#),
            ("end_life", r#"{"last_words":"再见！"}"#),
        ]),
        end_life("还是再见。"),
        Play::Says("好了。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    client
        .call("n", "subscribe", json!({"stream": "notices"}))
        .await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    // 结果照工具名、字排（`support::mcp::results`）：比几种各几条。
    let mut got = answers(&home, &session);
    got.sort();
    let required = ("last_words is required.".to_string(), true);
    let already = ("This life has already ended.".to_string(), false);
    let mut wanted = vec![
        required.clone(),
        required,
        already.clone(),
        already,
        (ENDED.to_string(), false),
    ];
    wanted.sort();
    assert_eq!(got, wanted, "没写的两次拒，记下一次，别的两次已经结束了");
    let log = home.log(&session);
    assert_eq!(ended(&log).len(), 1, "只记一条：{log:?}");
    assert!(notice(&mut client, Duration::from_secs(30)).await.is_some());
    assert!(
        notice(&mut client, Duration::from_millis(500))
            .await
            .is_none(),
        "只推一次"
    );
    assert!(life_row(&core, &session).is_none());
}

/// 验收第 3 条：没写遗言的拒了以后什么都没变：人生照旧在跑，那一行还在。验收第 5 条：`stop` 落在人生循环上回新的那一句。
#[tokio::test]
async fn a_refused_end_leaves_the_life_running_and_stop_points_to_end_life() {
    let home = Home::new();
    let script = Script::new([
        end_life(""),
        Play::calls(&[("schedule_wakeup", r#"{"stop":true}"#)]),
        Play::Says("那就继续。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    assert_eq!(
        answers(&home, &session),
        [("last_words is required.".to_string(), true)]
    );
    let stop = results(&home, &session)
        .into_iter()
        .find(|(name, ..)| name == "schedule_wakeup")
        .expect("调了");
    assert_eq!(
        stop.1,
        "A life loop cannot be stopped. To end this life, use end_life."
    );
    assert!(ended(&home.log(&session)).is_empty(), "什么都没记");
    assert!(life_row(&core, &session).is_some(), "那一行还在");
    let status = client
        .call("st", "life.status", json!({"persona": "muse"}))
        .await;
    assert_eq!(status["result"]["state"], "running", "{status}");
}
