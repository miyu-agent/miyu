//! 人生的两样小账（施工 LF-14）：轮数一次加一、删那一行跟着删；没看过的记、列、清；以前建的库（同一个版本号、没有这两张
//! 表）开的时候补上；重开读得回原样。

use rusqlite::Connection;

use super::super::{Creator, Loop, NewSchedule, Opened, Schedules, Target, Unseen, When};
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

fn session(last: u8) -> SessionId {
    SessionId::parse(&format!("01a17000-0000-7000-8000-00000000000{last}")).unwrap()
}

/// 跑在一个会话上的人生那一行。
fn life() -> NewSchedule {
    NewSchedule {
        by: Creator::Person,
        when: When::At { at: at(1_000) },
        prompt: String::new(),
        target: Target::Session {
            session: session(1),
        },
        looping: Some(Loop::Life),
        next: Some(at(1_000)),
    }
}

fn opened(root: &DataRoot) -> Schedules {
    let (table, how) = Schedules::open(root);
    assert!(matches!(how, Opened::Created | Opened::Kept), "{how:?}");
    table.expect("开得了")
}

#[test]
fn rounds_count_up_by_one_and_go_with_the_row() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    let row = table.add(life(), at(1_000)).unwrap();
    assert_eq!(table.rounds(&row.id).unwrap(), 0, "还没推演过");
    assert_eq!(table.add_round(&row.id).unwrap(), 1);
    assert_eq!(table.add_round(&row.id).unwrap(), 2);
    drop(table);
    let table = opened(&root);
    assert_eq!(table.rounds(&row.id).unwrap(), 2, "重开读得回");
    assert!(table.remove(&row.id).unwrap());
    assert_eq!(table.rounds(&row.id).unwrap(), 0, "那一行删了，轮数跟着删");
}

#[test]
fn unseen_lives_are_listed_oldest_first_until_seen() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    assert!(table.unseen().unwrap().is_empty());
    let late = Unseen {
        session: session(2),
        persona: Some("muse".to_string()),
        at: at(9_000),
    };
    let early = Unseen {
        session: session(3),
        persona: None,
        at: at(5_000),
    };
    table.mark_unseen(&late).unwrap();
    table.mark_unseen(&early).unwrap();
    table.mark_unseen(&late).unwrap();
    drop(table);
    let table = opened(&root);
    assert_eq!(table.unseen().unwrap(), [early.clone(), late.clone()]);
    assert!(table.seen(&late.session).unwrap(), "原来记着");
    assert!(!table.seen(&late.session).unwrap(), "再清一次没有了");
    assert_eq!(table.unseen().unwrap(), [early], "只清那一个会话的");
}

#[test]
fn a_database_from_before_gets_the_two_tables() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    let row = table.add(life(), at(1_000)).unwrap();
    drop(table);
    let db = Connection::open(super::super::path(&root)).unwrap();
    db.execute_batch("DROP TABLE rounds; DROP TABLE life_unseen")
        .unwrap();
    drop(db);
    let table = opened(&root);
    assert_eq!(
        table.get(&row.id).unwrap().expect("原来的行照读").id,
        row.id
    );
    assert_eq!(table.add_round(&row.id).unwrap(), 1, "补上了，照常用");
    assert!(table.unseen().unwrap().is_empty());
}
