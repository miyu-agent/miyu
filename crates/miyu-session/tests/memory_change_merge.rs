//! 整理不碰她的变化（施工 LF-11，`docs/blueprint/memory.md` 第七条）：整理记忆的模型是一台假服务器（`org/m`），人叫的那一次
//! （`Keeper::dream`）。只有新记的 `change` 的不起合并、不发请求；有别的新记的时，交给模型的请求里没有 `change`（新记的、相关的、
//! 轮到重看的都不算它），交回改、作废它的不落，摘要照写。

use std::sync::Arc;

use miyu_http::testkit::Server;
use miyu_kernel::origin::By;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{CHANGE, Entry, MemoryEvent, MemoryId, Merged, Saved, Summary, from_event};
use miyu_session::{Dreamed, Keeper, Listener, Turn};

use crate::support::extracting::*;
use crate::support::meaning::persona;
use crate::support::*;

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

fn her() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

fn module() -> By {
    serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法")
}

fn append(home: &Home, by: By, event: &MemoryEvent) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(now(), by, None, event).expect("记得下").id
}

fn save(home: &Home, class: &str, text: &str, by: By) -> MemoryId {
    let saved = Saved {
        class: class.into(),
        text: text.into(),
        sources: Vec::new(),
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
    append(home, by, &MemoryEvent::Saved(saved))
}

fn entry(home: &Home, id: MemoryId) -> Entry {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(|book| book.get(id).cloned()).expect("有这一条")
}

async fn dream(home: &Home) -> Dreamed {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(config, UtcOffset::from_minutes(540).expect("合写法"))
        .await
        .expect("整理了")
}

/// 这一间记下的最后一次开始合交了哪几条。
fn given(home: &Home) -> Vec<MemoryId> {
    keeper(home)
        .follow(Some(0), 1024)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Merging(merging) => Some(merging.given),
            _ => None,
        })
        .next_back()
        .expect("记了开始合")
}

/// 验收第 8 条：只有新记的 `change`、没有别的新条目时不起合并，模型那边一个请求都没有。
#[tokio::test]
async fn only_new_changes_start_no_merge() {
    let server = Server::start(vec![stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, CHANGE, "以前爱喝咖啡，这个月改喝茶", her());
    save(
        &home,
        CHANGE,
        "以前十一点就睡，这阵子常看书到十二点多",
        her(),
    );
    assert_eq!(dream(&home).await, Dreamed::default());
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(server.received().is_empty(), "没发请求");
}

/// 验收第 8 条：有别的新条目时，交给模型的请求里没有 `change`（字面对得上、会被当成相关的也没有）；交回改、作废它的不落，
/// 别的照落，摘要照写。
#[tokio::test]
async fn a_merge_never_sees_or_touches_a_change() {
    let answer = serde_json::json!({
        "revised": [
            {"id": "m1", "text": "她改喝茶了"},
            {"id": "m2", "text": "用户爱喝咖啡，每天两杯"}
        ],
        "retired": [{"id": "m1", "kind": "outdated", "why": "过时了"}],
        "summary": "用户爱喝咖啡。"
    });
    let server = Server::start(vec![stream(&answer.to_string())]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let change = save(&home, CHANGE, "以前爱喝咖啡，这个月改喝茶", her());
    let coffee = save(&home, "user", "用户爱喝咖啡", alice());
    let tea = save(&home, "user", "用户的同事爱喝茶", alice());
    let dreamed = dream(&home).await;
    assert_eq!((dreamed.revised, dreamed.retired), (1, 0), "{dreamed:?}");
    let asked = asked(&server, 0);
    assert!(!asked.contains("改喝茶"), "请求里没有变化：{asked}");
    assert!(!asked.contains(&format!("{change} ")), "{asked}");
    assert_eq!(given(&home), [coffee, tea]);
    let kept = entry(&home, change);
    assert!(kept.current(), "变化照旧：{kept:?}");
    assert_eq!(kept.text, "以前爱喝咖啡，这个月改喝茶");
    assert!(entry(&home, coffee).replaced_by.is_some(), "别的照落");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    assert_eq!(
        log.book(|book| book.summary().map(|(text, _)| text.to_string())),
        Some("用户爱喝咖啡。".to_string())
    );
}

/// 验收第 8 条：合过一次以后顺带重看旧记忆，轮不到 `change`。
#[tokio::test]
async fn the_sweep_skips_changes() {
    let server = Server::start(vec![stream(r#"{"summary": "用户住在东京。"}"#)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let change = save(
        &home,
        CHANGE,
        "以前十一点就睡，这阵子常看书到十二点多",
        her(),
    );
    let cat = save(&home, "user", "用户养了一只猫", alice());
    append(
        &home,
        module(),
        &MemoryEvent::Summary(Summary {
            text: "用户养了一只猫。".into(),
            upto: cat.seq(),
        }),
    );
    append(
        &home,
        module(),
        &MemoryEvent::Merged(Merged {
            upto: cat.seq(),
            given: 1,
            revised: 0,
            retired: 0,
            failed: false,
        }),
    );
    let tokyo = save(&home, "user", "用户搬到了东京", alice());
    dream(&home).await;
    let mut got = given(&home);
    got.sort();
    assert_eq!(got, [cat, tokyo], "新记的加轮到重看的猫，没有变化");
    assert!(!asked(&server, 0).contains("十二点多"));
    assert!(entry(&home, change).current());
}
