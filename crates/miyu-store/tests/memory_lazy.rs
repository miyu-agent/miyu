//! 记忆日志和记忆库用到才建（施工 V-28，`docs/blueprint/memory.md`「对外的样子」的记忆日志、记忆库）：一条都没写过的开了、
//! 读了，盘上什么都没有；第一次写记忆时建、重开搜得到；只有不进库的（人卡）的有日志、没有库，第一条记忆来了照底账整份放进去；
//! 原来就有的库照旧开；日志写了、库没来得及建就崩了的，重开照日志补上。

use std::fs;

use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_recall::{Card, MemoryEvent, MemoryId, Saved, Source};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::{Opened, Room};

use crate::support::*;

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

fn saved(text: &str) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: "user".into(),
        text: text.into(),
        sources: vec![Source {
            session: SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法"),
            turn: TurnId::new(Seq::new(3).expect("从 1 起")),
        }],
        audience: vec![serde_json::from_str(r#"{"kind":"person"}"#).expect("合写法")],
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
    })
}

fn card() -> MemoryEvent {
    MemoryEvent::Card(Card {
        who: "qq:10086".into(),
        text: "群里的老朋友".into(),
        names: Vec::new(),
    })
}

fn id(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

/// 人格 `engineer` 那一间的记忆库、记忆日志的目录。
fn places(root: &miyu_store::root::DataRoot) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        root.index().join("recall/memory-engineer.db"),
        root.path().join("modules/memory/engineer"),
    )
}

#[test]
fn nothing_is_created_until_the_first_memory_and_it_is_found_after_reopening() {
    let scratch = Scratch::new("memory-lazy-first");
    let root = root_in(&scratch);
    let (db, dir) = places(&root);
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&Room::persona("engineer")).expect("开得了");
    let report = report.expect("第一次开交回情形");
    assert!(report.index.is_none(), "{:?}", report.index);
    assert_eq!(log.search("猫", 10).expect("当空的读"), []);
    assert_eq!(log.book(|book| book.all().count()), 0);
    assert!(!db.exists() && !dir.exists(), "开了、读了，盘上什么都不建");

    let appended = log
        .append(at(1), tool(), None, &saved("用户养了一只橘猫"))
        .expect("记得下");
    assert_eq!(appended.id, id(1));
    assert!(appended.indexed.is_ok(), "{:?}", appended.indexed);
    assert!(
        matches!(appended.opened, Some(Opened::Created)),
        "{:?}",
        appended.opened
    );
    assert!(db.is_file() && dir.join("000000000001.jsonl").is_file());
    assert_eq!(log.search("橘猫", 10).expect("搜得了"), [id(1)]);
    let second = log
        .append(at(2), tool(), None, &saved("周末爱爬山"))
        .expect("记得下");
    assert!(second.opened.is_none(), "建过的不再建");
    drop((log, logs));

    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&Room::persona("engineer")).expect("开得了");
    let report = report.expect("第一次开交回情形");
    assert!(
        matches!(report.index, Some(Opened::Kept)),
        "{:?}",
        report.index
    );
    assert_eq!(log.search("橘猫", 10).expect("搜得了"), [id(1)]);
    assert_eq!(log.search("爬山", 10).expect("搜得了"), [id(2)]);
}

#[test]
fn a_log_without_memories_gets_no_index_until_one_arrives() {
    let scratch = Scratch::new("memory-lazy-card");
    let root = root_in(&scratch);
    let (db, dir) = places(&root);
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs.open(&Room::persona("engineer")).expect("开得了");
    let appended = log.append(at(1), tool(), None, &card()).expect("记得下");
    assert!(appended.opened.is_none() && appended.indexed.is_ok());
    assert!(dir.is_dir(), "写了一条，日志就在");
    assert!(!db.exists(), "人卡不进记忆库，不建库");
    drop((log, logs));

    // 重开：日志里没有要放进库的，照旧不建。
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&Room::persona("engineer")).expect("开得了");
    assert!(report.expect("第一次开").index.is_none());
    assert!(!db.exists());
    let appended = log
        .append(at(2), tool(), None, &saved("用户用 N 卡"))
        .expect("记得下");
    assert!(matches!(appended.opened, Some(Opened::Created)));
    assert_eq!(log.search("N卡", 10).expect("搜得了"), [id(2)]);
    assert!(db.is_file());
}

#[test]
fn an_existing_index_is_opened_as_before_even_with_an_empty_log() {
    let scratch = Scratch::new("memory-lazy-existing");
    let root = root_in(&scratch);
    let (db, dir) = places(&root);
    // 以前的版本开一次就建：空日志、空库。
    drop(miyu_store::recall::RecallIndex::open(&db));
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join("000000000001.jsonl"), "").expect("写得进");
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&Room::persona("engineer")).expect("开得了");
    assert!(
        matches!(report.expect("第一次开").index, Some(Opened::Kept)),
        "原来就有的照旧开"
    );
    let appended = log
        .append(at(1), tool(), None, &saved("用户养了一只橘猫"))
        .expect("记得下");
    assert!(appended.opened.is_none(), "开着的不再建");
    assert_eq!(log.search("橘猫", 10).expect("搜得了"), [id(1)]);
}

#[test]
fn a_crash_before_the_index_is_built_is_caught_up_from_the_log() {
    let scratch = Scratch::new("memory-lazy-crash");
    let root = root_in(&scratch);
    let (db, _) = places(&root);
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs.open(&Room::persona("engineer")).expect("开得了");
    log.append(at(1), tool(), None, &saved("用户养了一只橘猫"))
        .expect("记得下");
    drop((log, logs));
    // 日志落了盘、库没建成就没了（崩在两步之间）：库连同 -wal、-shm 都不在。
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db.clone().into_os_string();
        name.push(suffix);
        match fs::remove_file(&name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("{error}"),
        }
    }
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&Room::persona("engineer")).expect("开得了");
    let report = report.expect("第一次开");
    assert!(
        matches!(report.index, Some(Opened::Created)),
        "{:?}",
        report.index
    );
    assert_eq!(report.filled, 1, "照日志补上");
    assert_eq!(log.search("橘猫", 10).expect("搜得了"), [id(1)]);
}

#[test]
fn a_session_room_without_memories_leaves_nothing_in_the_session_directory() {
    let scratch = Scratch::new("memory-lazy-session");
    let root = root_in(&scratch);
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs.open(&Room::session(&session)).expect("开得了");
    assert_eq!(log.search("猫", 10).expect("当空的读"), []);
    assert!(!root.session_dir(&session).join("memory").exists());
}
