//! 写关键词（施工 R-15 下，`docs/blueprint/memory.md` 第二条）：真会话、真记忆日志、假的整理记忆的模型。抽取记下以后另发一次，
//! 给刚记下的几条写上，只收交进去的编号，照关键词搜得到；合并落完以后补这一间还没有的，最多二十条，有了的、作废的不交；写不成的
//! 不碍抽取。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_recall::{Entry, MemoryEvent, MemoryId, Retired, Saved};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

async fn session(home: &Home, turns: usize) -> Handle {
    home.create_full(
        &says(turns),
        &catalog(home),
        Opening::default(),
        Lines::default(),
    )
    .await
}

fn m(n: u64) -> MemoryId {
    MemoryId::parse(&format!("m{n}")).expect("合写法")
}

fn entry(home: &Home, id: MemoryId) -> Option<Entry> {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(|book| book.get(id).cloned())
}

/// 等 `id` 那一条有关键词（最多 10 秒）。
async fn until_tagged(home: &Home, id: MemoryId) -> String {
    for _ in 0..200 {
        if let Some(keywords) = entry(home, id).and_then(|entry| entry.keywords) {
            return keywords;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没写上 {id} 的关键词");
}

/// 写关键词交回的一段。
fn tagged(json: serde_json::Value) -> miyu_http::testkit::Reply {
    stream(&json.to_string())
}

#[tokio::test]
async fn what_extraction_saves_gets_keywords_right_after() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        tagged(serde_json::json!({"keywords": [
            {"id": "m1", "keywords": "宠物,猫咪"},
            {"id": "m9", "keywords": "没交进去的"}
        ]})),
    ])
    .await;
    let mut home = Home::new();
    organizer_tagging(&mut home, &server, "");
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    assert_eq!(until_tagged(&home, m(1)).await, "宠物,猫咪");
    let text = asked(&server, 1);
    assert!(text.starts_with("Below are memories you keep."), "{text}");
    assert!(text.contains("\nm1 user "), "一条一行：{text}");
    assert!(entry(&home, m(9)).is_none(), "没交进去的编号不收");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    assert_eq!(
        log.search("猫咪", 10).expect("搜得了"),
        [m(1)],
        "照关键词搜得到"
    );
}

#[tokio::test]
async fn a_failed_tagging_leaves_the_memory_as_it_was() {
    let server = Server::start(vec![every_turn("用户养了一只猫"), stream("不是 JSON")]).await;
    let mut home = Home::new();
    organizer_tagging(&mut home, &server, "");
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let saved = entry(&home, m(1)).expect("照样记下了");
    assert_eq!(saved.text, "用户养了一只猫");
    assert_eq!(saved.keywords, None, "写不成的不碍抽取");
}

/// 主人当场记的一条，`keywords` 照给的。
fn save_tagged(home: &Home, text: &str, keywords: &str) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
        venue: None,
        keywords: Some(keywords.into()),
        who: None,
        people: Vec::new(),
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

#[tokio::test]
async fn a_merge_fills_in_up_to_twenty_without_keywords() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        tagged(serde_json::json!({"keywords": []})),
        stream(
            &serde_json::json!({"revised": [], "retired": [], "summary": "用户养猫。"}).to_string(),
        ),
        tagged(serde_json::json!({"keywords": [{"id": "m25", "keywords": "旧的,补上"}]})),
    ])
    .await;
    let mut home = Home::new();
    organizer_tagging(&mut home, &server, "merge_sessions = 1\n");
    // 二十五条以前记的、没有关键词；一条有了的；一条作废了的。
    for n in 0..25 {
        save(&home, &format!("以前记的第 {n} 件事"));
    }
    save_tagged(&home, "有了关键词的一条", "已经有了");
    save(&home, "作废了的一条");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(
        now(),
        alice(),
        None,
        &MemoryEvent::Retired(Retired {
            id: m(27),
            why: "不要了".into(),
            kind: None,
            kept: None,
        }),
    )
    .expect("记得下");
    drop(log);
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 4).await;
    assert_eq!(until_tagged(&home, m(25)).await, "旧的,补上");
    let text = asked(&server, 3);
    assert!(text.starts_with("Below are memories you keep."), "{text}");
    let rows: Vec<&str> = text.lines().filter(|line| line.starts_with('m')).collect();
    assert_eq!(rows.len(), 20, "最多二十条：{text}");
    assert!(!text.contains("有了关键词的一条"), "有了的不交：{text}");
    assert!(!text.contains("作废了的一条"), "作废的不交：{text}");
    assert!(
        rows[0].starts_with("m30 user "),
        "这一次交进去的在前、新的在前：{text}"
    );
}

/// 落的时候还没有关键词的才写（施工 R-15 下）：模型还在写，人先改上了的，写好了也不盖。
#[tokio::test]
async fn keywords_given_meanwhile_are_not_overwritten() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        stream_after(
            &serde_json::json!({"keywords": [{"id": "m1", "keywords": "模型写的"}]}).to_string(),
            Duration::from_millis(800),
        ),
    ])
    .await;
    let mut home = Home::new();
    organizer_tagging(&mut home, &server, "");
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 2).await;
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(
        now(),
        alice(),
        None,
        &MemoryEvent::Tagged(miyu_recall::Tagged {
            id: m(1),
            keywords: "人写的".into(),
        }),
    )
    .expect("记得下");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        entry(&home, m(1))
            .and_then(|entry| entry.keywords)
            .as_deref(),
        Some("人写的"),
        "写好了也不盖"
    );
}
