//! `/life` 在不带人格的会话里（施工 LF-12，`docs/blueprint/life.md`「命令」第 8 条）：人生是照人格算的，这个会话没有人格就去找
//! 所有人格的。只有一个人格有没结束的人生会话的：光打回它的状态（带人格的名字）、带它的编号和人格，头照常跳过去；`pause`、
//! `resume`、`kill` 照它办。不止一个的不猜，列出来，什么都不动。`new` 不猜给谁开，回一句。一个都没有的回说明、回没有。
//!
//! 光打的 `/life`（带不带人格都是）多带 `lives`：所有人格没结束的人生会话，这个会话的人格的排最前（项目主人同一天第 29 条：
//! 「快速跳转人生会话」）；`life.list` 回同一份，不在会话里留痕。
//!
//! 起因是项目主人 2026-11-17 的本机验收第 25 条：人生是 `persona-1` 的，他在几个没有人格的会话里打 `/life`，回的是「这个会话
//! 没有人格，没有人生会话。」，找不到也进不去。

use serde_json::{Value, json};

use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::life_command::{
    GAP, START, asleep, carried, life_row, life_rows, offset, said, started, typed,
};
use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 没有人格、也没有哪个人格有人生会话时光打 `/life` 的说明。
const ABOUT: &str = "人生会话让一个人格一直过着自己的日子：到点自己醒来，做点自己的事，记下的会进这个人格的记忆。每次醒来都是一次模型请求，会多花 token。这个会话没有人格：先给它选一个人格，再用 /life new 开始。";

/// 没有人格的会话里 `/life new`。
const NO_NEW: &str = "这个会话没有人格，开不了新的人生会话：先给它选一个人格。";

/// 没有人格、也没有哪个人格有人生会话时停、续、结束的那一句。
const NOTHING: &str = "这个会话没有人格，也没有哪个人格有人生会话。";

/// 回应里带的人格：没带的是空的。
fn persona(reply: &Value) -> Option<&str> {
    reply["result"]["persona"].as_str()
}

/// 回应里的 `lives`：没带的是 `null`。
fn lives(reply: &Value) -> &Value {
    &reply["result"]["lives"]
}

/// `life.list` 的 `lives`。
async fn listed(client: &mut Client, id: &str) -> Value {
    let reply = client.call(id, "life.list", json!({})).await;
    reply["result"]["lives"].clone()
}

/// 给人格 `persona` 开一条人生，等她头一次醒完、睡下，交回会话编号。
async fn living(home: &Home, client: &mut Client, id: &str, persona: &str, turns: usize) -> String {
    let opened = client
        .call(id, "life.start", json!({"persona": persona}))
        .await;
    let session = opened["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("开出来：{opened}"))
        .to_string();
    home.until_turns(&session, turns).await;
    asleep(client, &session).await;
    session
}

/// 只有一个人格有人生会话：不带人格的会话里光打 `/life` 回它的状态，写上人格的名字，带它的编号和人格；停、续、结束都照它办。
#[tokio::test]
async fn a_session_without_a_persona_reaches_the_only_life() {
    let home = Home::new();
    home.write(
        "root/personas/muse/persona.toml",
        "[persona]\nname = \"缪斯\"\n",
    );
    let script = Script::new([Play::Says("醒了。"), Play::Says("又醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;
    let bare = client.create("c1", "~").await;
    let session = living(&home, &mut client, "l1", "muse", 1).await;

    let at = Timestamp::from_unix_millis(START + GAP).expect("在范围里");
    let next = at.local_minute(offset());
    let one = json!([{
        "state": "running", "session": session, "next": at, "persona": "muse", "name": "缪斯",
        "rounds": 1,
    }]);
    let reply = typed(&mut client, "k1", &bare, "/life").await;
    assert_eq!(
        said(&reply),
        format!("缪斯 的人生会话进行中，下次醒来：{next}。")
    );
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert_eq!(persona(&reply), Some("muse"), "{reply}");
    assert_eq!(lives(&reply), &one, "{reply}");
    assert_eq!(listed(&mut client, "q1").await, one, "`life.list` 同一份");

    // 带人格的会话里回执照旧不写名字，回应同样带上是哪个人格的、同一份列表。
    let reply = typed(&mut client, "k2", &here, "/life").await;
    assert_eq!(said(&reply), format!("人生会话进行中，下次醒来：{next}。"));
    assert_eq!(persona(&reply), Some("muse"), "{reply}");
    assert_eq!(lives(&reply), &one, "{reply}");

    let reply = typed(&mut client, "k3", &bare, "/life pause").await;
    assert_eq!(
        said(&reply),
        "缪斯 的人生会话已暂停，用 /life resume 继续。"
    );
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert_eq!(persona(&reply), Some("muse"), "{reply}");
    assert!(lives(&reply).is_null(), "只有光打的带列表：{reply}");
    assert!(life_row(&core, &session).expect("那一行在").paused, "停了");
    assert_eq!(listed(&mut client, "q2").await[0]["state"], "paused");

    // 续上就是马上醒：等她这一次醒完再看。
    let reply = typed(&mut client, "k4", &bare, "/life resume").await;
    assert!(
        said(&reply).starts_with("缪斯 的人生会话进行中，"),
        "{reply}"
    );
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert!(!life_row(&core, &session).expect("那一行在").paused, "续了");
    home.until_turns(&session, 2).await;
    asleep(&mut client, &session).await;

    let reply = typed(&mut client, "k5", &bare, "/life kill").await;
    assert_eq!(
        said(&reply),
        "缪斯 的人生会话已结束，留下的记忆已清掉，会话保留为只读。"
    );
    assert_eq!(carried(&reply), Some(session.as_str()), "{reply}");
    assert_eq!(persona(&reply), Some("muse"), "{reply}");
    assert!(life_rows(&core).is_empty(), "那一行删了");

    // 结束了的算没有。
    let reply = typed(&mut client, "k6", &bare, "/life").await;
    assert_eq!(said(&reply), ABOUT);
    assert_eq!(carried(&reply), None, "{reply}");
    assert_eq!(persona(&reply), None, "{reply}");
    assert!(lives(&reply).is_null(), "一条都没有的不写：{reply}");
    assert_eq!(listed(&mut client, "q3").await, json!([]));
}

/// 一个人格都没有人生会话：光打回说明（写明这个会话没有人格、先选一个），`new` 不开，停、续、结束回没有；什么都不动。
#[tokio::test]
async fn without_a_persona_and_without_any_life_nothing_is_opened() {
    let home = Home::new();
    let script = Script::new([]);
    let hand = Hand::at(START);
    let (core, mut client, _here) = started(&home, &script, &hand).await;
    let bare = client.create("c1", "~").await;

    let reply = typed(&mut client, "b1", &bare, "/life").await;
    assert_eq!(said(&reply), ABOUT);
    assert_eq!(carried(&reply), None, "{reply}");
    let reply = typed(&mut client, "b2", &bare, "/life new").await;
    assert_eq!(said(&reply), NO_NEW);
    assert_eq!(carried(&reply), None, "{reply}");
    for (id, text) in [
        ("b3", "/life pause"),
        ("b4", "/life resume"),
        ("b5", "/life kill"),
    ] {
        let reply = typed(&mut client, id, &bare, text).await;
        assert_eq!(said(&reply), NOTHING, "{text}");
        assert_eq!(carried(&reply), None, "{text}：{reply}");
    }
    assert!(life_rows(&core).is_empty(), "什么都没开");
}

/// 不止一个人格有人生会话：不猜是哪个，照人格的编号列出来（在跑的、停着的各写各的），不带会话；停、结束也只列、不动；
/// `new` 照旧回开不了。
#[tokio::test]
async fn with_several_lives_none_is_guessed() {
    let home = Home::new();
    home.write("root/personas/echo/prompts/persona.md", "You are Echo.\n");
    let script = Script::new([Play::Says("醒了。"), Play::Says("我也醒了。")]);
    let hand = Hand::at(START);
    let (core, mut client, here) = started(&home, &script, &hand).await;
    let bare = client.create("c1", "~").await;
    let muse = living(&home, &mut client, "l1", "muse", 1).await;
    let echo = living(&home, &mut client, "l2", "echo", 1).await;
    client
        .call("p1", "life.pause", json!({"persona": "muse"}))
        .await;

    let several = "这个会话没有人格，有人生会话的人格不止一个：echo（进行中），muse（已暂停）。到其中一个人格的会话里再用 /life。";
    for (id, text) in [
        ("s1", "/life"),
        ("s2", "/life pause"),
        ("s3", "/life resume"),
        ("s4", "/life kill"),
    ] {
        let reply = typed(&mut client, id, &bare, text).await;
        assert_eq!(said(&reply), several, "{text}");
        assert_eq!(carried(&reply), None, "{text}：{reply}");
        assert_eq!(persona(&reply), None, "{text}：{reply}");
        // 光打的带列表，头照它让人挑：照人格的编号排。
        let order: Vec<(&str, &str, &str)> = lives(&reply)
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|one| {
                Some((
                    one["persona"].as_str()?,
                    one["state"].as_str()?,
                    one["session"].as_str()?,
                ))
            })
            .collect();
        match text {
            "/life" => assert_eq!(
                order,
                [
                    ("echo", "running", echo.as_str()),
                    ("muse", "paused", muse.as_str())
                ],
                "{reply}"
            ),
            _ => assert!(order.is_empty(), "{text}：{reply}"),
        }
    }
    // 带人格的会话里：照旧是它自己的那一条（回执、要跳过去的），列表里它排最前。
    let reply = typed(&mut client, "s6", &here, "/life").await;
    assert_eq!(said(&reply), "人生会话已暂停，用 /life resume 继续。");
    assert_eq!(carried(&reply), Some(muse.as_str()), "{reply}");
    let order: Vec<&str> = lives(&reply)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|one| one["persona"].as_str())
        .collect();
    assert_eq!(order, ["muse", "echo"], "{reply}");
    let reply = typed(&mut client, "s5", &bare, "/life new").await;
    assert_eq!(said(&reply), NO_NEW);
    assert!(life_row(&core, &muse).expect("那一行在").paused, "还停着");
    assert!(!life_row(&core, &echo).expect("那一行在").paused, "还在跑");
    assert_eq!(life_rows(&core).len(), 2, "没多没少");
}

/// 英文、日文的回执都有。
#[tokio::test]
async fn bare_receipts_come_in_english_and_japanese() {
    let cases = [
        (
            "en-US",
            "A life session lets a persona live its own days: it wakes up on its own, does its own things, and what it notes goes into its memory. Every wake-up is a model request and costs extra tokens. This session has no persona: give it one first, then start with /life new.",
            "This session has no persona, so a new life session cannot be started here: give it a persona first.",
            "This session has no persona, and no persona has a life session.",
        ),
        (
            "ja-JP",
            "人生セッションでは、ペルソナが自分の日々を過ごします。時間になると自分で目を覚まし、自分のことをして、書き留めたことは記憶に残ります。目を覚ますたびにモデルへのリクエストが1回あり、そのぶんトークンを消費します。このセッションにはペルソナがありません。先にペルソナを選んでから /life new で始めてください。",
            "このセッションにはペルソナがないため、新しい人生セッションを始められません。先にペルソナを選んでください。",
            "このセッションにはペルソナがなく、人生セッションのあるペルソナもありません。",
        ),
    ];
    for (locale, about, no_new, nothing) in cases {
        let home = Home::new();
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
        let bare = client.create("c0", "~").await;
        for (id, text, want) in [
            ("k1", "/life", about),
            ("k2", "/life new", no_new),
            ("k3", "/life pause", nothing),
        ] {
            let reply = typed(&mut client, id, &bare, text).await;
            assert_eq!(said(&reply), want, "{locale} {text}");
        }
    }
}
