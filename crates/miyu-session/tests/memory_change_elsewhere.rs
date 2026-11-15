//! 她的变化别处不带（施工 LF-11，`docs/blueprint/memory.md` 第三条第 9 款、第四条、第六条）：真会话、真记忆日志。联想不带
//! `change`（人说的话正好对得上那一条的字也不带，对得上的别的类照带）；人生会话醒来交新记的那一块不带，常驻那一块列的 `change`
//! 不拉低下限；抽取时告诉模型「忘掉了别再记」的那一段不列她去掉的变化；抽取交回类 `change` 的不记。

use std::sync::Arc;

use miyu_http::testkit::Server;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_recall::associate::Thresholds;
use miyu_recall::{CHANGE, MemoryEvent, MemoryId, Retired, Saved, Source, Summary};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Embedder, Handle, Keeper, Listener, RecallTexts, Turn, Using, Vectors};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, filled, model_data, persona};
use crate::support::package::tiny_package;
use crate::support::*;

const MODEL: &str = "local:tiny";

/// 记下的时刻：会话的时区（东九区）里是 10 月 8 日。
fn saved_at() -> Timestamp {
    Timestamp::parse("2026-10-07T20:00:00.000Z").expect("合写法")
}

fn her() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

/// `by` 记一条 `class`，出处是 `from` 的第 1 轮（没有的没有出处），交回编号。
fn save(home: &Home, class: &str, text: &str, by: By, from: Option<&SessionId>) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: class.into(),
        text: text.into(),
        sources: from
            .map(|session| Source {
                session: session.clone(),
                turn: TurnId::new(Seq::new(1).expect("从 1 起")),
            })
            .into_iter()
            .collect(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
        why: None,
        imported: None,
    };
    log.append(saved_at(), by, None, &MemoryEvent::Saved(saved))
        .expect("记得下")
        .id
}

fn retire(home: &Home, id: MemoryId, by: By) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let retired = MemoryEvent::Retired(Retired {
        id,
        why: "淡了".into(),
        kind: None,
        kept: None,
    });
    log.append(now(), by, None, &retired).expect("记得下");
}

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 日志里模块注入的 `kind` 那一类的几块：原文和 `refs`。
fn injected(log: &[Event], kind: &str) -> Vec<(String, Vec<String>)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == kind => {
                Some((fact.text.clone(), fact.refs.clone()))
            }
            _ => None,
        })
        .collect()
}

/// 验收第 7 条：会话开着时别处记下一条 `change` 和一条字面对得上的 `user`，人说的话正好是那一条变化的字：联想带 `user`，不带
/// `change`。
#[tokio::test]
async fn recall_never_brings_a_change() {
    let home = Home::new();
    let setup = tiny_package(&home.scratch.0.join("packages").join("embed"));
    let vectors = Vectors::new(Some(Embedder::new(setup)), model_data(&home));
    assert!(home.memory.give_vectors(Arc::new(vectors)));
    let thresholds =
        Thresholds::parse(&format!("[thresholds]\n\"{MODEL}\" = -1.0\n")).expect("合写法");
    let texts = RecallTexts::with(home.resources.path(), thresholds).expect("读得到");
    assert!(home.memory.give_recall(texts));
    save(&home, "user", "用户住在东京", alice(), None);
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你好").await;
    let words = "以前爱喝咖啡，这个月改喝茶";
    let change = save(&home, CHANGE, words, her(), None);
    let tea = save(
        &home,
        "user",
        "用户这个月改喝茶，以前爱喝咖啡",
        alice(),
        None,
    );
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
    };
    Keeper::new(&home.memory, persona(), Listener::Owner(alice())).fill(&using);
    for id in [change, tea] {
        filled(&home, MODEL, true, &id.to_string()).await;
    }
    let log = chat(&home, &handle, 2, words).await;
    let brought: Vec<String> = injected(&log, "recall")
        .into_iter()
        .flat_map(|(_, refs)| refs)
        .collect();
    assert!(
        brought.contains(&tea.to_string()),
        "对得上的别的类照带：{brought:?}"
    );
    assert!(
        !brought.contains(&change.to_string()),
        "变化不带：{brought:?}"
    );
    stop(&handle).await;
}

fn life() -> By {
    serde_json::from_str(r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#).expect("合写法")
}

async fn wake(handle: &Handle, id_text: &str) {
    let mut pushes = watch(handle).await;
    within(
        &format!("命令 {id_text} 的回应"),
        handle.command(id(id_text), life(), say("醒来，过你的日子。")),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一个别的会话：在跑过、目录在，出处在它里面的算活的。
async fn elsewhere(home: &Home) -> SessionId {
    let other = home.create(&says(1)).await;
    let session = other.id().clone();
    stop(&other).await;
    session
}

/// 验收第 7 条：人生会话醒来交新记的那一块不带别处记下的 `change`，别的照交。
#[tokio::test]
async fn a_life_wake_brings_no_change() {
    let home = Home::new();
    save(&home, "user", "用户养了一只猫", alice(), None);
    let handle = home.create(&says(2)).await;
    wake(&handle, "cmd-1").await;
    let other = elsewhere(&home).await;
    let change = save(
        &home,
        CHANGE,
        "以前爱喝咖啡，这个月改喝茶",
        her(),
        Some(&other),
    );
    let trip = save(&home, "episode", "下午去了海边", alice(), Some(&other));
    wake(&handle, "cmd-2").await;
    let fresh = injected(&home.log(handle.id()), "fresh");
    assert_eq!(fresh.len(), 1, "{fresh:?}");
    assert_eq!(fresh[0].1, [trip.to_string()], "只有别的类");
    assert!(!fresh[0].0.contains("咖啡"));
    assert!(!fresh[0].1.contains(&change.to_string()));
    stop(&handle).await;
}

/// 常驻那一块列的 `change` 编号比摘要合到的小，不拉低醒来那一块的下限：摘要里合过的不再交（施工 LF-11 跟着改的一处）。
#[tokio::test]
async fn an_old_change_in_the_summary_keeps_the_wake_floor() {
    let home = Home::new();
    save(
        &home,
        CHANGE,
        "以前十一点就睡，这阵子常看书到十二点多",
        her(),
        None,
    );
    let cat = save(&home, "user", "用户养了一只猫", alice(), None);
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let module: By = serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法");
    let summary = MemoryEvent::Summary(Summary {
        text: "用户养了一只猫。".into(),
        upto: cat.seq(),
    });
    log.append(saved_at(), module, None, &summary)
        .expect("记得下");
    let handle = home.create(&says(2)).await;
    wake(&handle, "cmd-1").await;
    let other = elsewhere(&home).await;
    let trip = save(&home, "episode", "下午去了海边", alice(), Some(&other));
    wake(&handle, "cmd-2").await;
    let fresh = injected(&home.log(handle.id()), "fresh");
    assert_eq!(fresh.len(), 1, "{fresh:?}");
    assert_eq!(fresh[0].1, [trip.to_string()], "摘要合过的猫不再交");
    stop(&handle).await;
}

/// `software/memory/extract/dropped.txt`。
const DROPPED: &str = "These memories were forgotten or changed after this conversation, so what the conversation says about them is out of date. Do not record them again:";

/// 验收第 7 条：这一段开始以后她去掉的变化不列进「忘掉了别再记」那一段，同时作废的别的类照列。
#[tokio::test]
async fn extraction_is_not_told_about_dropped_changes() {
    let server = Server::start(vec![every_turn("不要紧")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let change = save(&home, CHANGE, "以前爱喝咖啡，这个月改喝茶", her(), None);
    let checkup = save(&home, "user", "用户下周三上午去体检", alice(), None);
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "下周三我要干嘛").await;
    retire(&home, change, her());
    retire(&home, checkup, alice());
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    let (_, after) = text
        .split_once(&format!("\n\n{DROPPED}\n"))
        .unwrap_or_else(|| panic!("作废的别的类照列：{text}"));
    let lines: Vec<&str> = after.lines().collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with(&format!("{checkup} user ")),
        "{lines:?}"
    );
    assert!(!after.contains("咖啡"), "{after}");
    stop(&handle).await;
}

/// 验收第 4 条：抽取交回类 `change` 的不记（抽取照出厂的几类查），这一段照样记抽到了哪。
#[tokio::test]
async fn extraction_never_saves_a_change() {
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .map(|turn| serde_json::json!({"class": "change", "text": "以前爱喝咖啡，现在喝茶", "turn": turn}))
        .collect();
    let reply = stream(&serde_json::json!({ "memories": memories }).to_string());
    let server = Server::start(vec![reply]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我这个月改喝茶了").await;
    chat(&home, &handle, 2, "好的").await;
    requested(&server, 1).await;
    for _ in 0..200 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(mark(&home, handle.id()).is_some(), "抽完了");
    assert!(
        extracted(&home).is_empty(),
        "一条都没记：{:?}",
        extracted(&home)
    );
    stop(&handle).await;
}
