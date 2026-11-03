//! 列出会话里主人问过的话（施工 SP-2，`docs/blueprint/protocol.md`「`session.prompts`」）：真核心走一遍。每一条是有效历史里
//! 主人本人说的 `message.user`（桥代主人在私聊里转进来的、带 `via` 的也算）：撤销掉的不列、恢复的放回、重做照最后那一版；命令
//! 模板展开的照人敲的那一行；子会话的、平台上别的人的、模块注入的、别的 harness 的、定时的都不列。每一次都和准绳对一遍：整份 `read_events` → `History::whole`
//! → 有效历史里主人本人的 `message.user` → 照会话表 `preview` 的取法取字。坏行、没有这个会话、参数不对、缺能力的拒，同
//! `view.page`。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;
use miyu_tool::Catalog;

use crate::support::extensions::{Program, install_asking, quick, read, record, steps};
use crate::support::*;

/// 问话的字最多几个字。
const CHARS: usize = 100;

async fn prompts(client: &mut Client, id: &str, session: &str) -> Value {
    client
        .call(id, "session.prompts", json!({"session": session}))
        .await
}

/// 准绳：整份读、整份交给留着一切的有效历史，取主人本人说的；闲着时说的照它开的那一轮记回合。
fn oracle(log: Vec<Event>) -> Value {
    let mut history = History::whole();
    for event in log {
        history.append(event);
    }
    let events = history.events();
    let listed: Vec<Value> = events
        .iter()
        .filter_map(|event| {
            let Body::MessageUser(message) = &event.body else {
                return None;
            };
            if !matches!(&event.by, By::Person(_)) {
                return None;
            }
            let turn = event.turn.or_else(|| {
                // 它开的那一轮在它后面（触发写成还没有的序号的那种，不算）。
                events.iter().find_map(|started| match &started.body {
                    Body::TurnStarted(body)
                        if body.trigger == Some(event.seq) && started.seq > event.seq =>
                    {
                        started.turn
                    }
                    _ => None,
                })
            });
            let mut item = json!({
                "seq": event.seq,
                "at": event.at,
                "text": miyu_store::index::said(message, CHARS),
            });
            if let Some(turn) = turn {
                item["turn"] = json!(turn);
            }
            Some(item)
        })
        .collect();
    json!({"prompts": listed})
}

/// 回应和准绳对一遍，交回列出来的字。
async fn checked(client: &mut Client, home: &Home, id: &str, session: &str) -> Vec<String> {
    let reply = prompts(client, id, session).await;
    let dir = home
        .root
        .session_dir(&SessionId::parse(session).expect("合写法"));
    let log = read_events(&dir).expect("读得出");
    assert_eq!(reply["result"], oracle(log), "{reply}");
    reply["result"]["prompts"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .map(|item| item["text"].as_str().unwrap_or_default().to_string())
        .collect()
}

const REVIEW: &str = "---\ndescription: Review a file\n---\nReview $ARGUMENTS carefully.\n";

#[tokio::test]
async fn it_lists_what_the_person_asked_with_typed_commands_as_typed() {
    let home = Home::new();
    home.write("root/commands/review.md", REVIEW);
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    assert_eq!(
        prompts(&mut client, "p0", &session).await["result"],
        json!({"prompts": []}),
        "还没人说过话的是空的"
    );
    client.say("s1", &session, "\n  第一句  \n第二行").await;
    home.until_turns(&session, 1).await;
    let ran = client
        .call(
            "k1",
            "command.run",
            json!({"session": session, "text": "/review src/a.rs"}),
        )
        .await;
    assert!(ran.get("error").is_none(), "{ran}");
    home.until_turns(&session, 2).await;
    let long = "长".repeat(150);
    client.say("s2", &session, &long).await;
    home.until_turns(&session, 3).await;
    let texts = checked(&mut client, &home, "p1", &session).await;
    assert_eq!(
        texts,
        [
            "第一句".to_string(),
            "/review src/a.rs".to_string(),
            "长".repeat(CHARS)
        ]
    );
    let reply = prompts(&mut client, "p2", &session).await;
    let first = &reply["result"]["prompts"][0];
    let said = home
        .log(&session)
        .into_iter()
        .find(|event| matches!(event.body, Body::MessageUser(_)))
        .expect("说过");
    assert_eq!(first["seq"], json!(said.seq));
    assert_eq!(first["at"], json!(said.at), "时刻同别处的写法");
    assert!(
        first["turn"].as_u64().is_some(),
        "闲着时说的照它开的那一轮：{reply}"
    );
}

#[tokio::test]
async fn revert_unrevert_and_redo_follow_the_effective_history() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("三。"),
        Play::Says("改了。"),
    ]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (n, text) in ["a", "b", "c"].iter().enumerate() {
        client.say(&format!("s{n}"), &session, text).await;
        home.until_turns(&session, n + 1).await;
    }
    assert_eq!(
        checked(&mut client, &home, "p1", &session).await,
        ["a", "b", "c"]
    );

    let reverted = client
        .call("r1", "session.revert", json!({"session": session}))
        .await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    assert_eq!(
        checked(&mut client, &home, "p2", &session).await,
        ["a", "b"],
        "撤掉的不列"
    );
    let back = client
        .call("r2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(back.get("error").is_none(), "{back}");
    assert_eq!(
        checked(&mut client, &home, "p3", &session).await,
        ["a", "b", "c"],
        "恢复的放回"
    );
    let redone = client
        .call(
            "r3",
            "session.redo",
            json!({"session": session, "text": "c 改过"}),
        )
        .await;
    assert!(redone.get("error").is_none(), "{redone}");
    home.until_turns(&session, 4).await;
    assert_eq!(
        checked(&mut client, &home, "p4", &session).await,
        ["a", "b", "c 改过"],
        "编辑重发照最后那一版"
    );
}

/// 分叉出来的会话：只列它自己日志里的，底子里父会话的问话不列（它们在父会话的日志里，`view.page` 也翻不到）。
#[tokio::test]
async fn a_fork_lists_only_its_own() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "主线").await;
    home.until_turns(&session, 1).await;
    let forked = client
        .call("f1", "session.fork", json!({"session": session}))
        .await;
    let branch = forked["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{forked}"))
        .to_string();
    assert_eq!(
        checked(&mut client, &home, "p1", &branch).await,
        Vec::<String>::new()
    );
    client.say("s2", &branch, "支线").await;
    home.until_turns(&branch, 1).await;
    assert_eq!(checked(&mut client, &home, "p2", &branch).await, ["支线"]);
    assert_eq!(checked(&mut client, &home, "p3", &session).await, ["主线"]);
}

/// 会话目录里写一份日志（一段，第一条序号 1）。
fn written(home: &Home, id: &str, lines: &[String], tail: &str) -> String {
    let id = SessionId::parse(id).expect("合写法");
    let dir = home.root.session_dir(&id);
    std::fs::create_dir_all(&dir).expect("建得了");
    let mut text = lines.join("\n");
    text.push('\n');
    text.push_str(tail);
    std::fs::write(dir.join("000000000001.jsonl"), text).expect("写得进");
    id.as_str().to_string()
}

/// 第 `seq` 条：种类、回合、谁、命令编号、正文。
fn line(seq: u64, kind: &str, turn: Option<u64>, by: &str, body: &str) -> String {
    let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
    format!(
        r#"{{"seq":{seq},"at":"2026-11-03T08:00:{seq:02}.000Z","kind":"{kind}"{turn},"by":{by},"cause":"c{seq}","body":{body}}}"#
    )
}

fn text(words: &str) -> String {
    format!(r#"{{"blocks":[{{"type":"text","text":"{words}"}}]}}"#)
}

const PERSON: &str = r#"{"kind":"person"}"#;
const VIA: &str = r#"{"kind":"person","via":"qq:10001"}"#;
const CREATED: &str = r#"{"venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 一份什么都有的日志：主人本人说的三句（闲着时说的一句、回合里排着的一句、桥代主人在私聊里转进来的一句），别的来路各一句。
fn mixed() -> Vec<String> {
    let others = [
        // 平台上的人（群里别人说的）。
        r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#,
        // 桥代主人说的（私聊里认出是主人）：列。
        VIA,
        // 子会话、父会话发来的。
        r#"{"kind":"session","id":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c"}"#,
        // 模块注入的（长期目标的续轮）。
        r#"{"kind":"module","id":"goal"}"#,
        // 别的 harness。
        r#"{"kind":"harness","name":"claude"}"#,
        // 到点的定时任务。
        r#"{"kind":"schedule","schedule":"t1"}"#,
        r#"{"kind":"kernel"}"#,
    ];
    let mut lines = vec![
        line(1, "session.created", None, PERSON, CREATED),
        line(2, "message.user", None, PERSON, &text("本人闲着时说的")),
        line(
            3,
            "turn.started",
            Some(3),
            r#"{"kind":"kernel"}"#,
            r#"{"trigger":2}"#,
        ),
        line(4, "message.user", Some(3), PERSON, &text("本人排着的")),
    ];
    for by in others {
        let seq = lines.len() as u64 + 1;
        let words = if by == VIA {
            "主人在私聊里说的"
        } else {
            "别人"
        };
        lines.push(line(seq, "message.user", Some(3), by, &text(words)));
    }
    let seq = lines.len() as u64 + 1;
    lines.push(line(
        seq,
        "child.reported",
        None,
        r#"{"kind":"session","id":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c"}"#,
        r#"{"job":"j2","session":"01a0cf12-7e40-7d1b-8c3a-51d49f03b21c","reason":"done","text":"子会话回报"}"#,
    ));
    let seq = lines.len() as u64 + 1;
    lines.push(line(
        seq,
        "turn.ended",
        Some(3),
        r#"{"kind":"kernel"}"#,
        r#"{"reason":"completed"}"#,
    ));
    lines
}

#[tokio::test]
async fn only_the_persons_own_words_are_listed() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    // 最后半行还没写完：不算。
    let session = written(
        &home,
        "01a0e000-0000-7000-8000-000000000001",
        &mixed(),
        r#"{"seq":99,"kind":"message.us"#,
    );
    let reply = prompts(&mut client, "p1", &session).await;
    assert_eq!(
        reply["result"],
        json!({"prompts": [
            {"seq": 2, "turn": 3, "at": "2026-11-03T08:00:02.000Z", "text": "本人闲着时说的"},
            {"seq": 4, "turn": 3, "at": "2026-11-03T08:00:04.000Z", "text": "本人排着的"},
            {"seq": 6, "turn": 3, "at": "2026-11-03T08:00:06.000Z", "text": "主人在私聊里说的"},
        ]}),
        "{reply}"
    );
    let log = read_events(
        &home
            .root
            .session_dir(&SessionId::parse(&session).expect("合写法")),
    )
    .expect("读得出");
    assert_eq!(reply["result"], oracle(log));
}

#[tokio::test]
async fn bad_lines_missing_sessions_and_bad_params_are_refused_like_view_page() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let mut lines = mixed();
    lines[3] = r#"{"seq":4,"kind":"message.user","body":"#.to_string();
    let broken = written(&home, "01a0e000-0000-7000-8000-000000000002", &lines, "");
    let missing = "01a0e000-0000-7000-8000-000000000003".to_string();
    for (n, (params, reason)) in [
        (json!({"session": broken}), "session_broken"),
        (json!({"session": missing}), "session_not_found"),
        (json!({"session": "nope"}), "bad_params"),
        (json!({}), "bad_params"),
        (json!({"session": broken, "turns": 3}), "bad_params"),
    ]
    .into_iter()
    .enumerate()
    {
        let ours = client
            .call(&format!("p{n}"), "session.prompts", params.clone())
            .await;
        assert_eq!(ours["error"]["data"]["reason"], reason, "{params}：{ours}");
        // 同样的会话、同样的错，`view.page` 也这么回（多写的格各报各的）。
        if n < 3 {
            let page = client
                .call(&format!("v{n}"), "view.page", params.clone())
                .await;
            assert_eq!(page["error"], ours["error"], "{params}");
        }
    }
}

/// 核心拉起的扩展：同 `view.page` 要 `events.read`，没批的回 `needs_approval`。
#[tokio::test]
async fn an_extension_needs_events_read_like_view_page() {
    let home = Home::new();
    let program = Program::new();
    let asks = [
        r#"ask:session.prompts:{"session":"x"}"#,
        r#"ask:view.page:{"session":"x"}"#,
    ];
    let (path, keep) = record(&home, "plain");
    let mut all = vec![keep.as_str(), "hello"];
    all.extend_from_slice(&asks);
    all.push("wait");
    install_asking(
        &home,
        "plain",
        &program.name(),
        "always",
        &steps(&all),
        &["tools"],
    );
    crate::support::extensions::approve(&home, "plain", &["tools"]);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let replies = recorded(&path, 2 + asks.len()).await;
    for reply in &replies[1..] {
        assert_eq!(
            reply["error"]["data"]["reason"], "needs_approval",
            "{reply}"
        );
        assert_eq!(
            reply["error"]["data"]["capabilities"],
            json!(["events.read"])
        );
    }
}

/// 等记的文件里有 `n` 行（头一行是工作目录）。
async fn recorded(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let text = read(path);
        if text.lines().count() >= n {
            return text
                .lines()
                .skip(1)
                .map(|line| serde_json::from_str(line).expect("回应是 JSON"))
                .collect();
        }
        assert!(tokio::time::Instant::now() < deadline, "{text}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
