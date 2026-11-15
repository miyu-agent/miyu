//! 她记下的自己的变化（施工 LF-11，设计 `70-推演人生.md` 第六节）：不在出厂的几类名单里；改一条、作废一条、恢复一条 `change`
//! 都不让摘要作废，照被动到的那一条看（改一条时是被改的旧的那一条）；别的类照旧让摘要作废。

use super::*;

fn see(book: &mut MemoryBook, n: u64, event: MemoryEvent) {
    let event = to_event(seq(n), at(1), owner(), &event).expect("写得出");
    book.see(&event).expect("读得懂");
}

fn m(n: u64) -> MemoryId {
    MemoryId::new(seq(n))
}

fn change(text: &str, replaces: Option<MemoryId>) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: CHANGE.to_string(),
        ..saved(text, replaces)
    })
}

fn retired(n: u64) -> MemoryEvent {
    MemoryEvent::Retired(Retired {
        id: m(n),
        why: "淡了".into(),
        kind: None,
        kept: None,
    })
}

/// 一条 `change`（m1）、一条 `user`（m2），摘要合到 m2（m3）。
fn summarized() -> MemoryBook {
    let mut book = MemoryBook::default();
    see(
        &mut book,
        1,
        change("以前十一点睡，这阵子常看书到十二点多", None),
    );
    see(&mut book, 2, MemoryEvent::Saved(saved("养了一只猫", None)));
    see(
        &mut book,
        3,
        MemoryEvent::Summary(Summary {
            text: "她养了一只猫。".into(),
            upto: seq(2),
        }),
    );
    book
}

#[test]
fn change_is_not_one_of_the_classes_anyone_else_can_write() {
    assert_eq!(CHANGE, "change");
    assert!(!CLASSES.contains(&CHANGE));
    assert_eq!(CHANGES, 10);
}

#[test]
fn updating_retiring_or_restoring_a_change_keeps_the_summary() {
    let mut book = summarized();
    see(&mut book, 4, change("现在常看书到一点", Some(m(1))));
    assert_eq!(book.summary(), Some(("她养了一只猫。", seq(2))), "改了");
    assert!(!book.summary_stale());
    see(&mut book, 5, retired(4));
    assert_eq!(book.summary(), Some(("她养了一只猫。", seq(2))), "作废了");
    see(&mut book, 6, MemoryEvent::Restored(Restored { id: m(4) }));
    assert_eq!(book.summary(), Some(("她养了一只猫。", seq(2))), "恢复了");
    assert!(!book.summary_stale());
    assert_eq!(
        book.get(m(1)).map(|entry| entry.replaced_by),
        Some(Some(m(4)))
    );
}

#[test]
fn touching_another_class_under_the_summary_still_makes_it_stale() {
    let mut book = summarized();
    see(&mut book, 4, retired(2));
    assert_eq!(book.summary(), None);
    assert!(book.summary_stale());
}

/// 改一条照被改的那一条的类看：新写的一条是 `change`、改的是摘要里的 `user` 的（照规矩写不出来，底账照样算），摘要照旧作废。
#[test]
fn replacing_looks_at_the_replaced_entry_not_the_new_one() {
    let mut book = summarized();
    see(&mut book, 4, change("改成一条变化", Some(m(2))));
    assert_eq!(book.summary(), None);
    assert!(book.summary_stale());
}
