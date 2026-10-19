//! 恢复作废的（施工 R-12 下，`memory.md` 第二条第 6 款）：`ext.memory.restored` 读写得回；底账里作废的那一条不算作废了，
//! 没有的那一条不碍事；恢复了摘要合进去的一条，摘要不算了，摘要以后才记的不碍事。

use super::*;

fn see(book: &mut MemoryBook, n: u64, event: MemoryEvent) {
    let event = to_event(seq(n), at(1), admin(), &event).expect("写得出");
    book.see(&event).expect("读得懂");
}

fn m(n: u64) -> MemoryId {
    MemoryId::new(seq(n))
}

fn retired(n: u64) -> MemoryEvent {
    MemoryEvent::Retired(Retired {
        id: m(n),
        why: "错了".into(),
        kind: None,
        kept: None,
    })
}

fn restored(n: u64) -> MemoryEvent {
    MemoryEvent::Restored(Restored { id: m(n) })
}

#[test]
fn a_restoration_round_trips() {
    let event = to_event(seq(5), at(1), admin(), &restored(3)).expect("写得出");
    assert_eq!(event.body.kind(), "ext.memory.restored");
    assert!(
        event.to_line().ends_with(r#""body":{"id":"m3"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(restored(3))));
}

#[test]
fn a_restored_memory_counts_again() {
    let mut book = MemoryBook::default();
    see(&mut book, 1, MemoryEvent::Saved(saved("用 N 卡", None)));
    see(&mut book, 2, retired(1));
    assert!(!book.get(m(1)).expect("在").current());
    see(&mut book, 3, restored(1));
    let entry = book.get(m(1)).expect("在");
    assert_eq!(entry.retired, None, "不算作废了");
    assert!(entry.current());
    see(&mut book, 4, restored(9));
    assert!(book.get(m(9)).is_none(), "没有的那一条不碍事");
}

#[test]
fn restoring_what_a_summary_covered_makes_it_stale() {
    let mut book = MemoryBook::default();
    see(&mut book, 1, MemoryEvent::Saved(saved("用 N 卡", None)));
    see(&mut book, 2, MemoryEvent::Saved(saved("养了一只猫", None)));
    see(&mut book, 3, retired(1));
    let summary = |upto: u64| {
        MemoryEvent::Summary(Summary {
            text: "用户养了一只猫。".into(),
            upto: seq(upto),
        })
    };
    see(&mut book, 4, summary(3));
    assert!(book.summary().is_some());
    see(&mut book, 5, restored(1));
    assert!(book.summary().is_none(), "恢复了它合进去的一条");
    assert!(book.summary_stale(), "下一次合并从头来");

    see(&mut book, 6, summary(5));
    see(&mut book, 7, MemoryEvent::Saved(saved("住在杭州", None)));
    see(&mut book, 8, retired(7));
    see(&mut book, 9, restored(7));
    assert!(book.summary().is_some(), "摘要以后才记的恢复不碍事");
}
