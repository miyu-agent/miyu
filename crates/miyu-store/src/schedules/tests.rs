//! 唤醒表（施工 J-1 上）：建、读、改、删，编号不复用；到点的、最早的照时刻挑，停着的不算；坏了的改名留着开空的，比这一版
//! 新的不动；重开读得回原样。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

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

fn alice() -> AccountId {
    AccountId::parse("alice").unwrap()
}

fn at(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).unwrap()
}

fn session() -> SessionId {
    SessionId::parse("01a17000-0000-7000-8000-000000000001").unwrap()
}

/// 每隔一分钟、跑在 `session()` 上、人建的一个，第一次在 `next`。
fn every_minute(next: i64) -> NewSchedule {
    NewSchedule {
        by: Creator::Person { account: alice() },
        when: When::Every { every: 60 },
        prompt: "看看 CI".to_string(),
        target: Target::Session { session: session() },
        looping: None,
        next: Some(at(next)),
    }
}

fn opened(root: &DataRoot) -> Schedules {
    let (table, opened) = Schedules::open(root, &alice());
    assert!(
        matches!(opened, Opened::Created | Opened::Kept),
        "{opened:?}"
    );
    table.expect("开得了")
}

#[test]
fn a_schedule_is_written_read_changed_and_removed() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    let first = table.add(every_minute(2_000), at(1_000)).unwrap();
    assert_eq!(first.id, "t1");
    assert_eq!(first.created, at(1_000));
    assert!(!first.paused && first.last.is_none());
    let mut second = table
        .add(
            NewSchedule {
                by: Creator::Model { session: session() },
                when: When::Cron {
                    cron: "0 8 * * *".to_string(),
                    recurring: false,
                    zone: None,
                },
                prompt: "早报".to_string(),
                target: Target::New {
                    persona: None,
                    preset: "none".to_string(),
                    cwd: "/w".to_string(),
                },
                looping: Some(Loop::Dynamic),
                next: None,
            },
            at(1_500),
        )
        .unwrap();
    assert_eq!(second.id, "t2");
    assert_eq!(table.get("t2").unwrap().as_ref(), Some(&second));
    second.paused = true;
    second.last = Some(Last {
        at: at(3_000),
        result: "skipped:preset_off".to_string(),
        session: Some(session()),
    });
    assert!(table.put(&second).unwrap());
    assert_eq!(table.get("t2").unwrap().as_ref(), Some(&second), "改的都在");
    assert!(table.remove("t1").unwrap());
    assert!(!table.remove("t1").unwrap(), "没有的交回假");
    assert_eq!(table.get("t1").unwrap(), None);
    let third = table.add(every_minute(9_000), at(4_000)).unwrap();
    assert_eq!(third.id, "t3", "删了的号不再用");
    let mut gone = third.clone();
    gone.id = "t9".to_string();
    assert!(!table.put(&gone).unwrap(), "没有的交回假");
}

#[test]
fn due_and_earliest_follow_the_next_time_and_skip_paused_ones() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let table = opened(&root);
    assert_eq!(table.earliest().unwrap(), None);
    let late = table.add(every_minute(5_000), at(0)).unwrap();
    let early = table.add(every_minute(3_000), at(0)).unwrap();
    let mut paused = table.add(every_minute(1_000), at(0)).unwrap();
    paused.paused = true;
    table.put(&paused).unwrap();
    let mut waiting = table.add(every_minute(0), at(0)).unwrap();
    waiting.next = None;
    table.put(&waiting).unwrap();
    assert_eq!(table.earliest().unwrap(), Some(at(3_000)), "停着的不算");
    let ids = |list: Vec<Schedule>| list.into_iter().map(|one| one.id).collect::<Vec<_>>();
    assert_eq!(ids(table.due(at(2_999)).unwrap()), Vec::<String>::new());
    assert_eq!(
        ids(table.due(at(5_000)).unwrap()),
        [early.id.clone(), late.id.clone()]
    );
    assert_eq!(
        ids(table.list().unwrap()),
        [early.id, late.id, paused.id, waiting.id],
        "有下一次的照时刻在前，停着的、没定的在后"
    );
}

#[test]
fn a_reopened_table_reads_back_the_same() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let written = {
        let table = opened(&root);
        table.add(every_minute(2_000), at(1_000)).unwrap()
    };
    let (table, opened) = Schedules::open(&root, &alice());
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    let table = table.unwrap();
    assert_eq!(table.list().unwrap(), [written]);
    assert_eq!(table.path(), path(&root, &alice()));
    assert!(
        table.path().ends_with(
            std::path::Path::new("alice")
                .join("state")
                .join("schedules.db")
        )
    );
}

#[test]
fn a_broken_table_is_set_aside_and_a_newer_one_left_alone() {
    let scratch = Scratch::new();
    let root = root(&scratch);
    let file = path(&root, &alice());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        "not a database at all, but long enough to be read as one",
    )
    .unwrap();
    let (table, opened) = Schedules::open(&root, &alice());
    let Opened::SetAside { kept, .. } = opened else {
        panic!("{opened:?}");
    };
    assert_eq!(
        fs::read_to_string(&kept).unwrap(),
        "not a database at all, but long enough to be read as one",
        "原样留着"
    );
    let table = table.expect("开了一份空的");
    assert_eq!(table.list().unwrap(), []);
    drop(table);

    let other = AccountId::parse("bob").unwrap();
    let file = path(&root, &other);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    {
        let db = Connection::open(&file).unwrap();
        db.execute_batch("CREATE TABLE future (x); PRAGMA user_version = 99;")
            .unwrap();
    }
    let before = fs::read(&file).unwrap();
    let (table, opened) = Schedules::open(&root, &other);
    assert!(matches!(opened, Opened::Newer(99)), "{opened:?}");
    assert!(table.is_none(), "这一回不用");
    assert_eq!(fs::read(&file).unwrap(), before, "没动它");
}
