//! 核心命令 `/life`（施工 LF-5 下，`docs/blueprint/life.md`「命令」）：真核心、手拨的钟，命令经 `command.run` 发。照打命令的
//! 会话的人格调 `life.*` 那一组：光打 `/life` 只说状态、没有的不开只说明；`new`、`pause`、`resume`、`kill` 同那几个方法；回应
//! 多带一格 `session`（人生会话的编号，有才带）。在人生会话自己里面打照样管用；不带人格、场所会话、认不出的参数各回各的、
//! 什么都不动；回执三种语言都有，命令目录里有它。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Loop, Schedule, Target};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::origin::By;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::venues::bound_core;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 默认间隔：一小时。
const GAP: i64 = 3_600_000;

/// 没有人生会话时的说明（中文）。
const ABOUT: &str = "人生会话让这个人格一直过着自己的日子：到点自己醒来，做点自己的事，记下的会进这个人格的记忆。每次醒来都是一次模型请求，会多花 token。用 /life new 开始。";

/// 没有人生会话时停、续、结束的那一句。
const NOTHING: &str = "这个人格没有人生会话。";

/// 起好唤醒表的核心、握好手的连接，和一个人格是 `muse` 的本机会话（打命令的那一个）。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client, String) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let core = core_with_tools(home, script, hand, Catalog::default());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let here = client.create_as("c0", "~", "muse").await;
    (core, client, here)
}

/// 在会话 `session` 里打 `text`。
async fn typed(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

/// 回应里的 `said`。
fn said(reply: &Value) -> &str {
    reply["result"]["said"]
        .as_str()
        .unwrap_or_else(|| panic!("没成：{reply}"))
}

/// 回应里带的人生会话编号：没带的是空的。
fn carried(reply: &Value) -> Option<&str> {
    reply["result"]["session"].as_str()
}

/// `life.status` 的回应。
async fn status(client: &mut Client, id: &str) -> Value {
    client
        .call(id, "life.status", json!({"persona": "muse"}))
        .await["result"]
        .clone()
}

/// 定时表里全部的人生那一行。
fn life_rows(core: &Core) -> Vec<Schedule> {
    core.schedules()
        .list()
        .expect("读得了")
        .into_iter()
        .filter(|one| one.looping == Some(Loop::Life))
        .collect()
}

/// 跑在 `session` 上的人生那一行。
fn life_row(core: &Core, session: &str) -> Option<Schedule> {
    life_rows(core).into_iter().find(
        |one| matches!(&one.target, Target::Session { session: on } if on.as_str() == session),
    )
}

/// 人生循环叫醒了几次。
fn woken(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| {
            matches!(
                (&event.by, &event.body),
                (By::Schedule(_), Body::MessageUser(_))
            )
        })
        .count()
}

/// 日志里的 `life.ended`。
fn ended(log: &[Event]) -> Vec<&Event> {
    log.iter()
        .filter(|event| matches!(event.body, Body::LifeEnded(_)))
        .collect()
}

/// 本机这时的时区，同核心（`sessions::offset`）。
fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes).expect("在范围里")
}

/// 光打 `/life` 而没有人生会话：只回说明，不开、不建那一行、回应不带 `session`；上一条结束了的同没有。
#[tokio::test]
async fn bare_life_without_one_only_explains_and_opens_nothing() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;

    let reply = typed(&mut client, "k1", &here, "/life").await;
    assert_eq!(reply["result"]["command"], "life", "{reply}");
    assert_eq!(said(&reply), ABOUT);
    assert_eq!(carried(&reply), None, "{reply}");
    assert_eq!(status(&mut client, "s1").await, json!({"state": "none"}));
    assert!(life_rows(&core).is_empty(), "没建人生那一行");

    // 上一条结束了的：同没有。
    let opened = client
        .call("l1", "life.start", json!({"persona": "muse"}))
        .await;
    let session = opened["result"]["session"]
        .as_str()
        .expect("开出来")
        .to_string();
    home.until_turns(&session, 1).await;
    let killed = client
        .call("x1", "life.kill", json!({"persona": "muse"}))
        .await;
    assert_eq!(killed["result"]["state"], "ended", "{killed}");
    let reply = typed(&mut client, "k2", &here, "/life").await;
    assert_eq!(said(&reply), ABOUT);
    assert_eq!(carried(&reply), None, "{reply}");
    assert_eq!(
        status(&mut client, "s2").await,
        json!({"state": "ended", "session": session}),
        "没开新的"
    );
}

/// 开着的：光打 `/life` 回状态（在跑的说下次几点醒，停着的说停着），带它的编号。
#[tokio::test]
async fn bare_life_with_one_tells_its_state_and_carries_it() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client, here) = started(&home, &script, &hand).await;
    let session = client
        .call("l1", "life.start", json!({"persona": "muse"}))
        .await["result"]["session"]
        .as_str()
        .expect("开出来")
        .to_string();
    home.until_turns(&session, 1).await;

    let reply = typed(&mut client, "k1", &here, "/life").await;
    let next = Timestamp::from_unix_millis(START + GAP)
        .expect("在范围里")
        .local_minute(offset());
    assert_eq!(said(&reply), format!("人生会话进行中，下次醒来：{next}。"));
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");

    client
        .call("p1", "life.pause", json!({"persona": "muse"}))
        .await;
    let reply = typed(&mut client, "k2", &here, "/life").await;
    assert_eq!(said(&reply), "人生会话已暂停，用 /life resume 继续。");
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
}

/// `/life new`：没有的开一条、她马上醒；有一条在跑的，旧的结束（人记的 `life.ended`）、新的开出来，是两个会话，回应带新的。
#[tokio::test]
async fn new_opens_one_and_again_ends_the_old_and_opens_another() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。"), Play::Says("又是新的一段。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;

    let reply = typed(&mut client, "k1", &here, "/life new").await;
    assert_eq!(said(&reply), "人生会话已开始，马上醒来。");
    let first = carried(&reply).expect("带新会话").to_string();
    home.until_turns(&first, 1).await;
    assert_eq!(woken(&home.log(&first)), 1, "马上醒了第一次");
    assert_eq!(
        status(&mut client, "s1").await["session"],
        json!(first),
        "就是这一条"
    );

    let reply = typed(&mut client, "k2", &here, "/life new").await;
    assert_eq!(
        said(&reply),
        "上一段人生会话已结束，新的一段已开始，马上醒来。"
    );
    let second = carried(&reply).expect("带新会话").to_string();
    assert_ne!(first, second, "是两个会话");
    let log = home.log(&first);
    let gone = ended(&log);
    assert_eq!(gone.len(), 1, "旧的结束了：{log:?}");
    assert!(matches!(gone[0].by, By::Person(_)), "{:?}", gone[0].by);
    assert!(life_row(&core, &first).is_none(), "旧的那一行没了");
    home.until_turns(&second, 1).await;
    assert_eq!(status(&mut client, "s2").await["session"], json!(second));
    assert_eq!(life_rows(&core).len(), 1, "只有新的那一行");
}

/// `/life pause` 以后到点不醒，`/life resume` 以后接着醒；没有人生会话时两个都回没有、不带 `session`。
#[tokio::test]
async fn pause_sleeps_resume_wakes_and_without_one_both_say_none() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。"), Play::Says("又醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;
    for (id, text) in [("n1", "/life pause"), ("n2", "/life resume")] {
        let reply = typed(&mut client, id, &here, text).await;
        assert_eq!(said(&reply), NOTHING, "{text}");
        assert_eq!(carried(&reply), None, "{text}：{reply}");
    }
    assert!(life_rows(&core).is_empty(), "什么都没开");

    let session = carried(&typed(&mut client, "k1", &here, "/life new").await)
        .expect("开出来")
        .to_string();
    home.until_turns(&session, 1).await;
    let reply = typed(&mut client, "k2", &here, "/life pause").await;
    assert_eq!(said(&reply), "人生会话已暂停，用 /life resume 继续。");
    assert_eq!(carried(&reply), Some(session.as_str()));
    hand.advance(3 * GAP);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(woken(&home.log(&session)), 1, "停着的到点不醒");

    let reply = typed(&mut client, "k3", &here, "/life resume").await;
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert!(
        said(&reply).starts_with("人生会话进行中"),
        "{}",
        said(&reply)
    );
    home.until_turns(&session, 2).await;
    assert_eq!(woken(&home.log(&session)), 2, "续上接着醒");
}

/// `/life kill`：结束（人记的 `life.ended`、封存），回应带那个会话；没有的回没有。
#[tokio::test]
async fn kill_ends_it_and_without_one_says_none() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;
    let reply = typed(&mut client, "n1", &here, "/life kill").await;
    assert_eq!(said(&reply), NOTHING);
    assert_eq!(carried(&reply), None);

    let session = carried(&typed(&mut client, "k1", &here, "/life new").await)
        .expect("开出来")
        .to_string();
    home.until_turns(&session, 1).await;
    let reply = typed(&mut client, "k2", &here, "/life kill").await;
    assert_eq!(
        said(&reply),
        "人生会话已结束，留下的记忆已清掉，会话保留为只读。"
    );
    assert_eq!(carried(&reply), Some(session.as_str()));
    let log = home.log(&session);
    let gone = ended(&log);
    assert_eq!(gone.len(), 1, "{log:?}");
    assert!(matches!(gone[0].by, By::Person(_)), "{:?}", gone[0].by);
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::SessionSealed(_))),
        "封存了"
    );
    assert!(life_row(&core, &session).is_none());

    // 结束了的：再结束回没有。
    let reply = typed(&mut client, "n2", &here, "/life kill").await;
    assert_eq!(said(&reply), NOTHING);
    assert_eq!(carried(&reply), None);
}

/// 在人生会话自己里面打：照它自己的人格，状态、结束都回得出来（结束了这个会话也封存了）；之后在这个只读的会话里还能
/// `/life new` 开下一段。
#[tokio::test]
async fn inside_the_life_session_itself_it_works_too() {
    let home = Home::new();
    let script = Script::new([Play::Says("醒了。"), Play::Says("新的一段。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;
    let session = carried(&typed(&mut client, "k1", &here, "/life new").await)
        .expect("开出来")
        .to_string();
    home.until_turns(&session, 1).await;

    let reply = typed(&mut client, "k2", &session, "/life").await;
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert!(
        said(&reply).starts_with("人生会话进行中"),
        "{}",
        said(&reply)
    );

    let reply = typed(&mut client, "k3", &session, "/life kill").await;
    assert_eq!(
        said(&reply),
        "人生会话已结束，留下的记忆已清掉，会话保留为只读。",
        "{reply}"
    );
    assert_eq!(carried(&reply), Some(session.as_str()));
    assert!(
        home.log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::SessionSealed(_))),
        "封存了"
    );

    let reply = typed(&mut client, "k4", &session, "/life new").await;
    let next = carried(&reply).expect("开出新的").to_string();
    assert_ne!(next, session);
    home.until_turns(&next, 1).await;
    assert!(life_row(&core, &next).is_some());
}

/// 不带人格的会话、场所会话、认不出的参数：各回各的那一句，什么都不动。
#[tokio::test]
async fn no_persona_a_venue_and_unknown_words_change_nothing() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;

    let bare = client.create("c1", "~").await;
    for (id, text) in [("b1", "/life"), ("b2", "/life new"), ("b3", "/life kill")] {
        let reply = typed(&mut client, id, &bare, text).await;
        assert_eq!(said(&reply), "这个会话没有人格，没有人生会话。", "{text}");
        assert_eq!(carried(&reply), None, "{text}：{reply}");
    }

    for (id, text) in [
        ("u1", "/life now"),
        ("u2", "/life new please"),
        ("u3", "/life confirm"),
    ] {
        let reply = typed(&mut client, id, &here, text).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{text}：{reply}");
        assert_eq!(
            reply["error"]["data"]["why"], "用法：/life [new | pause | resume | kill]",
            "{text}：{reply}"
        );
    }
    // 本机的会话里说命令的就是主人：`command.run` 不收 `as`，冒不成别人。
    let manager = json!({"external": "qq:10003", "role": "manager"});
    let reply = client
        .call(
            "m1",
            "command.run",
            json!({"session": here, "text": "/life new", "as": manager}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(status(&mut client, "s1").await, json!({"state": "none"}));
    assert!(life_rows(&core).is_empty(), "什么都没开");

    // 场所会话：照没有这条命令，主人本人也一样。
    let home = Home::new();
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    let mut client = Client::connect(bound_core(&home, &Script::new([]), Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let venue = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let owner = json!({"external": "qq:10001"});
    for (id, text) in [("v2", "/life"), ("v3", "/life new")] {
        let reply = client
            .call(
                id,
                "command.run",
                json!({"session": venue, "text": text, "as": owner}),
            )
            .await;
        assert_eq!(reason(&reply), Some("unknown_command"), "{text}：{reply}");
    }
}

/// 说明、状态的回执英文、日文都有；命令目录里有它，带参数提示和几种写法。
#[tokio::test]
async fn receipts_come_in_three_languages_and_the_catalog_lists_it() {
    let cases = [
        (
            "en-US",
            "A life session lets this persona live its own days: it wakes up on its own, does its own things, and what it notes goes into its memory. Every wake-up is a model request and costs extra tokens. Start one with /life new.",
            "This persona has no life session.",
            "Life session",
        ),
        (
            "ja-JP",
            "人生セッションでは、このペルソナが自分の日々を過ごします。時間になると自分で目を覚まし、自分のことをして、書き留めたことは記憶に残ります。目を覚ますたびにモデルへのリクエストが1回あり、そのぶんトークンを消費します。始めるには /life new。",
            "このペルソナには人生セッションがありません。",
            "人生セッション",
        ),
    ];
    for (locale, about, nothing, summary) in cases {
        let home = Home::new();
        home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
        let hand = Hand::at(START);
        let core = core_with_tools(&home, &Script::new([]), &hand, Catalog::default());
        let mut client = Client::connect(core);
        let hello = client
            .call(
                "hello-1",
                "hello",
                json!({
                    "protocol": [1, 1],
                    "head": {"kind": "test", "version": "0.0.0"},
                    "locale": locale,
                    "caps": {"input": true},
                    "token": TOKEN,
                }),
            )
            .await;
        assert!(hello.get("error").is_none(), "{hello}");
        let here = client.create_as("c0", "~", "muse").await;
        let reply = typed(&mut client, "k1", &here, "/life").await;
        assert_eq!(said(&reply), about, "{locale}");
        let reply = typed(&mut client, "k2", &here, "/life pause").await;
        assert_eq!(said(&reply), nothing, "{locale}");

        let listed = client
            .call("c1", "command.catalog", json!({"session": here}))
            .await;
        let life = listed["result"]["commands"]
            .as_array()
            .unwrap_or_else(|| panic!("{listed}"))
            .iter()
            .find(|one| one["name"] == "life")
            .unwrap_or_else(|| panic!("目录里有 life：{listed}"))
            .clone();
        assert_eq!(life["summary"], summary, "{locale}");
        assert_eq!(life["argument"], "[new | pause | resume | kill]");
        let forms: Vec<&str> = life["usage"]
            .as_array()
            .expect("有几种写法")
            .iter()
            .filter_map(|one| one["form"].as_str())
            .collect();
        assert_eq!(
            forms,
            [
                "/life",
                "/life new",
                "/life pause",
                "/life resume",
                "/life kill"
            ]
        );
    }
}
