//! 醒来时交新记的（施工 LF-6）：从哪条以后（常驻那一块、以前这种块列过的最新那一条；一条都没列过的照摘要合到哪；联想带过的
//! 不抬下限、只去重；人卡、别的模块不算），哪些回合（只有人生循环叫醒的）。

use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, Seq, SessionId, TurnId};
use miyu_kernel::origin::{By, Person, Schedule, ScheduleLoop};
use miyu_kernel::time::Timestamp;
use miyu_recall::{Entry, MemoryId, Source};

use super::{since, wanted, woken_by_life};

fn present(module: &str, kind: &str, refs: &[&str]) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: refs.iter().map(ToString::to_string).collect(),
    }
}

fn m(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).unwrap())
}

#[test]
fn the_floor_is_the_newest_the_summary_or_this_block_listed() {
    let (floor, seen) = since(
        &[
            present("memory", "memory", &["m10", "m4"]),
            present("memory", "card", &["qq:20017"]),
            present("memory", "fresh", &["m12", "m11"]),
            present("knowledge", "memory", &["m40"]),
        ],
        Some(m(3)),
    );
    assert_eq!(
        floor,
        Some(m(12)),
        "最新那一条，别的模块不算，摘要合到哪让位"
    );
    assert_eq!(
        seen,
        [m(4), m(10), m(11), m(12)].into(),
        "人卡的平台编号不算"
    );
}

#[test]
fn what_recall_brought_is_only_left_out_not_a_floor() {
    let (floor, seen) = since(
        &[
            present("memory", "memory", &["m10"]),
            present("memory", "recall", &["m15"]),
        ],
        None,
    );
    assert_eq!(floor, Some(m(10)), "联想带回很新的一条不抬下限");
    assert!(seen.contains(&m(15)), "它自己不重交");
}

#[test]
fn a_summary_with_only_the_digest_line_counts_from_where_the_digest_reached() {
    let (floor, _) = since(&[present("memory", "memory", &[])], Some(m(7)));
    assert_eq!(floor, Some(m(7)));
    let (floor, _) = since(&[present("memory", "memory", &[])], None);
    assert_eq!(floor, None, "都没有的没有下限");
}

#[test]
fn only_a_life_loop_wakes_it() {
    let schedule = |looping| {
        By::Schedule(Schedule {
            schedule: "t1".to_string(),
            looping,
        })
    };
    assert!(woken_by_life(Some(&schedule(Some(ScheduleLoop::Life)))));
    for other in [
        schedule(Some(ScheduleLoop::Dynamic)),
        schedule(Some(ScheduleLoop::Fixed)),
        schedule(None),
        By::Person(Person::default()),
    ] {
        assert!(!woken_by_life(Some(&other)), "{other:?}");
    }
    assert!(!woken_by_life(None));
}

fn session(n: u8) -> SessionId {
    SessionId::parse(&format!("0192f3a0-1111-7abc-8def-0011223344{n:02}")).unwrap()
}

/// 第 `n` 条，人记的（没有出处）。
fn entry(n: u64) -> Entry {
    Entry {
        id: m(n),
        class: "episode".into(),
        text: format!("第 {n} 件事"),
        sources: Vec::new(),
        audience: vec![By::Person(Person::default())],
        about: None,
        replaces: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
        at: Timestamp::parse("2026-10-07T20:00:00.000Z").unwrap(),
        by: By::Person(Person::default()),
        replaced_by: None,
        retired: None,
        retired_at: None,
        cleared: false,
    }
}

#[test]
fn above_the_floor_and_not_brought_before() {
    let here = session(1);
    let since = (Some(m(10)), [m(10), m(15)].into());
    assert!(!wanted(&entry(9), &since, &here), "下限以前的不交");
    assert!(!wanted(&entry(10), &since, &here), "下限那一条交过了");
    assert!(wanted(&entry(12), &since, &here), "比下限新、没交过的交");
    assert!(!wanted(&entry(15), &since, &here), "联想带过的不重交");
    assert!(
        wanted(&entry(1), &(None, [].into()), &here),
        "没有下限的都算"
    );
}

#[test]
fn what_she_lived_here_and_what_merging_made_are_not_wanted() {
    let (here, there) = (session(1), session(2));
    let since = (Some(m(1)), [].into());
    let from = |sessions: &[&SessionId]| Entry {
        sources: sessions
            .iter()
            .map(|session| Source {
                session: (*session).clone(),
                turn: TurnId::new(Seq::new(1).unwrap()),
            })
            .collect(),
        ..entry(5)
    };
    assert!(!wanted(&from(&[&here]), &since, &here), "全在这里的不交");
    assert!(!wanted(&from(&[&here, &here]), &since, &here));
    assert!(
        wanted(&from(&[&here, &there]), &since, &here),
        "有别处的照交"
    );
    assert!(wanted(&from(&[&there]), &since, &here));
    let merged = Entry {
        replaces: Some(m(2)),
        by: serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).unwrap(),
        ..entry(5)
    };
    assert!(!wanted(&merged, &since, &here), "合并自己改出来的不交");
    let revised = Entry {
        replaces: Some(m(2)),
        ..entry(5)
    };
    assert!(wanted(&revised, &since, &here), "人改的照交");
}
