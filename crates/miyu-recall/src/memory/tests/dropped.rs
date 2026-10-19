//! 什么时候忘掉的（施工 R-6 修，`memory.md` 第六条第 4 款）：作废的照作废那一条的时刻，恢复了的不算；改掉的旧版本照新版本记下
//! 的时刻；还算数的没有。

use super::*;

fn see(book: &mut MemoryBook, n: u64, when: i64, event: MemoryEvent) {
    let event = to_event(seq(n), at(when), admin(), &event).expect("写得出");
    book.see(&event).expect("读得懂");
}

fn m(n: u64) -> MemoryId {
    MemoryId::new(seq(n))
}

#[test]
fn a_memory_is_dropped_when_it_is_retired_or_replaced() {
    let mut book = MemoryBook::default();
    see(
        &mut book,
        1,
        1,
        MemoryEvent::Saved(saved("下周三去体检", None)),
    );
    see(
        &mut book,
        2,
        1,
        MemoryEvent::Saved(saved("养了一只猫", None)),
    );
    see(&mut book, 3, 1, MemoryEvent::Saved(saved("喜欢咖啡", None)));
    assert_eq!(book.dropped_at(m(1)), None, "还算数");
    see(
        &mut book,
        4,
        3,
        MemoryEvent::Retired(Retired {
            id: m(1),
            why: "不去了".into(),
            kind: None,
            kept: None,
        }),
    );
    assert_eq!(book.dropped_at(m(1)), Some(at(3)), "照作废那一条的时刻");
    see(
        &mut book,
        5,
        4,
        MemoryEvent::Restored(Restored { id: m(1) }),
    );
    assert_eq!(book.dropped_at(m(1)), None, "恢复了的不算");
    see(
        &mut book,
        6,
        5,
        MemoryEvent::Saved(saved("养了两只猫", Some(m(2)))),
    );
    assert_eq!(book.dropped_at(m(2)), Some(at(5)), "照新版本记下的时刻");
    assert_eq!(book.dropped_at(m(6)), None);
    assert_eq!(book.dropped_at(m(3)), None);
    assert_eq!(book.dropped_at(m(9)), None, "没有的");
}
