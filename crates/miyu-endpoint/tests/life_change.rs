//! 后天（施工 LF-11，设计 `70-推演人生.md` 第六节，`docs/blueprint/tools/note_change.md`）：真核心、手拨的钟。`note_change`
//! 只在没结束的人生会话的工具面上，预设里能单独关；她记下的变化 `memory_search` 搜得到、类是 `change`；她的 `remember`、
//! `forget` 碰不了它，协议写这一类、改它的拒，导入不收；主人作废、恢复照常。别的会话、人生结束以后在 `life_change_more.rs`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::request::Request;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::preset_face::tool_names;
use crate::support::clock::{Hand, core_with_tools};
use crate::support::mcp::results;
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

/// 记忆、循环、人生三个包的真工具。
pub(crate) fn catalog() -> Catalog {
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

/// 起好唤醒表的核心、握好手的连接。人格 `muse`、`sage` 写在数据根里。
pub(crate) async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client) {
    home.write("root/personas/muse/prompts/persona.md", "You are Muse.\n");
    home.write("root/personas/sage/prompts/persona.md", "You are Sage.\n");
    let core = core_with_tools(home, script, hand, catalog());
    core.start_schedules();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    (core, client)
}

pub(crate) async fn start(client: &mut Client, id: &str) -> String {
    let reply = client
        .call(id, "life.start", json!({"persona": "muse"}))
        .await;
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没开出来：{reply}"))
        .to_string()
}

/// 带工具面的第 `n` 次请求（从 0 数）。
pub(crate) fn request(script: &Script, n: usize) -> Request {
    script
        .requests()
        .into_iter()
        .map(|(_, request)| request)
        .filter(|request| !request.tools.is_empty())
        .nth(n)
        .unwrap_or_else(|| panic!("发了第 {n} 次请求"))
}

fn names(request: &Request) -> Vec<String> {
    tool_names(request)
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(crate) fn note(args: Value) -> Play {
    Play::calls(&[("note_change", &args.to_string())])
}

/// 会话 `session` 里工具 `name` 的回话，照工具名、字排（`support::mcp::results`）。
pub(crate) fn answers(home: &Home, session: &str, name: &str) -> Vec<(String, bool)> {
    results(home, session)
        .into_iter()
        .filter(|(tool, ..)| tool == name)
        .map(|(_, text, error, _)| (text, error))
        .collect()
}

/// 人格 `muse` 里类是 `change` 的（作废的也要），新的在前。
pub(crate) async fn changes(client: &mut Client, id: &str) -> Vec<Value> {
    let reply = client
        .call(
            id,
            "memory.list",
            json!({"persona": "muse", "class": "change", "forgotten": true}),
        )
        .await;
    reply["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone()
}

/// 验收第 3 条：人生会话第一轮就有 `note_change`、`end_life`；拿 `life` 预设开的普通会话、别的预设开了 `base-life` 的普通会话
/// 都没有，也不算进没开的。
#[tokio::test]
async fn only_a_life_session_gets_note_change() {
    let home = Home::new();
    home.write(
        "root/presets/mine.toml",
        "[preset]\nname = { en = \"Mine\" }\nunlisted = \"off\"\n\n[software]\nbase-life = true\nbase-memory = true\n",
    );
    let script = Script::new([Play::Says("醒了。"), Play::Says("嗯。"), Play::Says("好。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let first = names(&request(&script, 0));
    assert!(
        first.contains(&"note_change".to_string()) && first.contains(&"end_life".to_string()),
        "{first:?}"
    );
    for (n, preset) in ["life", "mine"].into_iter().enumerate() {
        let other = client
            .create_with(
                &format!("c{n}"),
                json!({"cwd": "~", "persona": "muse", "preset": preset}),
            )
            .await;
        client.say(&format!("s{n}"), &other, "hi").await;
        home.until_turns(&other, 1).await;
        let got = names(&request(&script, n + 1));
        assert!(
            !got.contains(&"note_change".to_string()),
            "{preset}：{got:?}"
        );
        assert!(!got.is_empty(), "{preset}：别的工具照给");
        let pin = crate::preset_face::snapshot(&home, &other)
            .preset
            .expect("记了预设");
        assert!(
            !pin.off.contains(&"change".to_string()),
            "{preset}：{:?}",
            pin.off
        );
    }
}

/// 验收第 3 条：预设里把功能 `change` 关了的人生会话里没有 `note_change`，`end_life` 还在。
#[tokio::test]
async fn the_change_feature_can_be_turned_off_alone() {
    let home = Home::new();
    home.write("root/presets/life.toml", "[features]\nchange = false\n");
    let script = Script::new([Play::Says("醒了。")]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let got = names(&request(&script, 0));
    assert!(!got.contains(&"note_change".to_string()), "{got:?}");
    assert!(got.contains(&"end_life".to_string()), "{got:?}");
}

/// 验收第 1、7 条：她记一条、改一条、去掉一条，出处是这一轮；`memory_search` 搜得到，行里的类是 `change`。
#[tokio::test]
async fn she_notes_updates_drops_and_finds_her_changes() {
    let home = Home::new();
    let script = Script::new([
        note(json!({"text": "以前十一点就睡，这阵子常看书到十二点多"})),
        note(json!({"text": "以前爱喝咖啡，这个月改喝茶"})),
        note(json!({"text": "这阵子常看书到一点", "replaces": "m1", "why": "书越看越长"})),
        note(json!({"drop": "m2", "why": "又喝回咖啡了"})),
        Play::calls(&[("memory_search", r#"{"query":"看书"}"#)]),
        Play::Says("记下了。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let mut wanted = vec![
        ("Noted as m1.".to_string(), false),
        ("Noted as m2.".to_string(), false),
        ("Noted as m3, in place of m1.".to_string(), false),
        ("Dropped m2.".to_string(), false),
    ];
    wanted.sort();
    assert_eq!(answers(&home, &session, "note_change"), wanted);
    let listed = changes(&mut client, "m").await;
    let shape: Vec<(Value, Value, Value)> = listed
        .iter()
        .map(|one| {
            (
                one["id"].clone(),
                one["text"].clone(),
                one["retired"].clone(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            (json!("m3"), json!("这阵子常看书到一点"), Value::Null),
            (
                json!("m2"),
                json!("以前爱喝咖啡，这个月改喝茶"),
                json!("又喝回咖啡了")
            ),
        ],
        "改掉的旧版本不列，作废的带为什么：{listed:?}"
    );
    assert_eq!(
        (listed[0].get("why"), listed[1].get("why")),
        (Some(&json!("书越看越长")), None),
        "她写了为什么变的那一条带着，没写的没有这一格：{listed:?}"
    );
    assert_eq!(
        listed[0]["sources"][0]["session"],
        json!(session),
        "出处是这一轮：{listed:?}"
    );
    let found = answers(&home, &session, "memory_search");
    assert!(
        found.len() == 1 && found[0].0.contains("m3 change ") && found[0].0.contains("看书到一点"),
        "{found:?}"
    );
}

/// 验收第 4 条：她的 `remember` 写类 `change`、`replaces` 指着它，`forget` 指着它，都拒；协议的 `memory.remember` 写这一类拒，
/// `memory.update` 改它拒成 `change_note`，`memory.import` 带这一类不收；主人作废、恢复照常。
#[tokio::test]
async fn other_ways_in_are_refused_and_the_owner_may_retire_and_restore() {
    let home = Home::new();
    let script = Script::new([
        note(json!({"text": "以前爱喝咖啡，这个月改喝茶"})),
        Play::calls(&[
            ("remember", r#"{"class":"change","text":"我变了"}"#),
            (
                "remember",
                r#"{"class":"user","text":"改成别的","replaces":"m1"}"#,
            ),
            ("forget", r#"{"id":"m1","why":"不像我"}"#),
        ]),
        Play::Says("好。"),
    ]);
    let hand = Hand::at(START);
    let (_core, mut client) = started(&home, &script, &hand).await;
    let session = start(&mut client, "l1").await;
    home.until_turns(&session, 1).await;
    let refused = "m1 is a note of how you have changed; it can only be updated or dropped from your life session.";
    let mut got = answers(&home, &session, "remember");
    got.extend(answers(&home, &session, "forget"));
    got.sort();
    let mut wanted = vec![
        (
            "Unknown class change. Classes are user, feedback, episode, reference, person."
                .to_string(),
            true,
        ),
        (refused.to_string(), true),
        (refused.to_string(), true),
    ];
    wanted.sort();
    assert_eq!(got, wanted);

    let reply = client
        .call(
            "r",
            "memory.remember",
            json!({"persona": "muse", "class": "change", "text": "人替她写"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = client
        .call(
            "u",
            "memory.update",
            json!({"persona": "muse", "id": "m1", "text": "人替她改"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("change_note"), "{reply}");
    let item = json!({"at": "2026-10-21T02:53:22.120Z", "class": "change", "text": "导进来的变化", "origin": "o:1"});
    let reply = client
        .call(
            "i",
            "memory.import",
            json!({"persona": "muse", "items": [item]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(reply["error"]["data"]["field"], "class", "{reply}");
    let listed = changes(&mut client, "l1").await;
    assert_eq!(listed.len(), 1, "只有她记的那一条：{listed:?}");
    assert_eq!(listed[0]["text"], "以前爱喝咖啡，这个月改喝茶");
    assert_eq!(listed[0]["retired"], Value::Null, "照旧算数");

    let reply = client
        .call(
            "f",
            "memory.forget",
            json!({"persona": "muse", "id": "m1", "why": "不像她"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(changes(&mut client, "l2").await[0]["retired"], "不像她");
    let reply = client
        .call(
            "re",
            "memory.restore",
            json!({"persona": "muse", "id": "m1"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(changes(&mut client, "l3").await[0]["retired"], Value::Null);
}
