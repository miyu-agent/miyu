//! 「现在在干嘛」那张表（施工 LF-7）：写、读、换、清；删任务那一行跟着删；以前建的库（同一个版本号、没有这张表）开的时候
//! 补上，原来的行照读；重开读得回原样。

use rusqlite::Connection;

use super::super::{Creator, Doing, Loop, NewSchedule, Opened, Schedules, Target, When};
use crate::env::{Env, Platform};
use crate::root::DataRoot;
use crate::test_support::Scratch;
use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

fn root(scratch: &Scratch) -> DataRoot {
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.rooted().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    DataRoot::locate(&env).unwrap()
}

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).unwrap()
}

/// 跑在一个会话上的人生那一行。
fn life() -> NewSchedule {
    NewSchedule {
        by: Creator::Person,
        when: When::At { at: at(1_000) },
        prompt: String::new(),
        target: Target::Session {
            session: SessionId::parse("01a17000-0000-7000-8000-000000000001").unwrap(),
        },
        looping: Some(Loop::Life),
        next: Some(at(1_000)),
    }
}

fn doing(text: &str, written: i64) -> Doing {
    Doing {
        text: text.to_string(),
        at: at(written),
        until: at(written + 600_000),
    }
}

fn opened(root: &DataRoot) -> Schedules {
    let (table, how) = Schedules::open(root);
    assert!(matches!(how, Opened::Created | Opened::Kept), "{how:?}");
    table.expect("开得了")
}

#[test]
fn what_she_is_doing_is_written_replaced_cleared_and_read_back_after_reopening() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    let row = table.add(life(), at(1_000)).unwrap();
    assert_eq!(table.doing(&row.id).unwrap(), None, "没写过的没有");

    table
        .set_doing(&row.id, Some(&doing("在看书", 2_000)))
        .unwrap();
    assert_eq!(table.doing(&row.id).unwrap(), Some(doing("在看书", 2_000)));
    table
        .set_doing(&row.id, Some(&doing("去散步", 3_000)))
        .unwrap();
    assert_eq!(
        table.doing(&row.id).unwrap(),
        Some(doing("去散步", 3_000)),
        "换成新的"
    );
    // 改那一行（到点写下一次）碰不到它。
    let mut ahead = table.get(&row.id).unwrap().unwrap();
    ahead.next = Some(at(9_000));
    assert!(table.put(&ahead).unwrap());
    drop(table);

    let table = opened(&root);
    assert_eq!(
        table.doing(&row.id).unwrap(),
        Some(doing("去散步", 3_000)),
        "重开读得回来，写下一次没盖掉它"
    );
    table.set_doing(&row.id, None).unwrap();
    assert_eq!(table.doing(&row.id).unwrap(), None, "清掉");
}

#[test]
fn removing_the_row_removes_what_she_was_doing() {
    let scratch = Scratch::new();
    let table = opened(&root(&scratch));
    let row = table.add(life(), at(1_000)).unwrap();
    let other = table.add(life(), at(1_000)).unwrap();
    table
        .set_doing(&row.id, Some(&doing("在看书", 2_000)))
        .unwrap();
    table
        .set_doing(&other.id, Some(&doing("在睡觉", 2_000)))
        .unwrap();
    assert!(table.remove(&row.id).unwrap());
    assert_eq!(table.doing(&row.id).unwrap(), None);
    assert_eq!(
        table.doing(&other.id).unwrap(),
        Some(doing("在睡觉", 2_000)),
        "别的那一行不动"
    );
    assert!(!table.remove(&row.id).unwrap(), "没有的交回假");
}

#[test]
fn an_older_table_without_it_gets_it_on_opening_and_keeps_its_rows() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let path = super::super::path(&root);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    {
        // LF-7 以前的结构，版本同是 1。
        let db = Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE counter (next INTEGER NOT NULL);
            INSERT INTO counter (next) VALUES (2);
            CREATE TABLE schedules (
                id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, by TEXT NOT NULL, when_json TEXT NOT NULL,
                prompt TEXT NOT NULL, target TEXT NOT NULL, target_session TEXT, loop TEXT,
                paused INTEGER NOT NULL DEFAULT 0, next_at INTEGER, last_at INTEGER, last_result TEXT,
                last_session TEXT
            );
            CREATE INDEX schedules_next ON schedules (next_at);
            CREATE INDEX schedules_session ON schedules (target_session);
            INSERT INTO schedules (id, created_at, by, when_json, prompt, target, target_session, loop, next_at)
            VALUES ('t1', 1000, '{\"kind\":\"person\"}', '{\"at\":{\"at\":\"1970-01-01T00:00:01.000Z\"}}', '',
                '{\"session\":{\"session\":\"01a17000-0000-7000-8000-000000000001\"}}',
                '01a17000-0000-7000-8000-000000000001', 'life', 1000);
            PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let (table, how) = Schedules::open(&root);
    assert!(matches!(how, Opened::Kept), "同一个版本照原样开：{how:?}");
    let table = table.unwrap();
    let rows = table.list().unwrap();
    assert_eq!(rows.len(), 1, "原来的行照读");
    assert_eq!(rows[0].looping, Some(Loop::Life));
    assert_eq!(table.doing("t1").unwrap(), None);
    table
        .set_doing("t1", Some(&doing("在看书", 2_000)))
        .unwrap();
    assert_eq!(table.doing("t1").unwrap(), Some(doing("在看书", 2_000)));
}
