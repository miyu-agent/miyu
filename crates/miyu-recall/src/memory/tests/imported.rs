//! 导进来的（施工 MI-1，`memory.md` 第二条、「协议」的 `memory.import`）：`ext.memory.saved` 的 `noted`、`origin` 读写得回、
//! 没有的不写（以前的日志照读）；底账一条的时刻照 `noted`，没有的照事件的时刻；导进来过的 `origin` 作废、改掉、清掉了也记着。

use super::*;

fn see(book: &mut MemoryBook, n: u64, event: MemoryEvent) {
    let event = to_event(seq(n), at(100), owner(), &event).expect("写得出");
    book.see(&event).expect("读得懂");
}

fn imported(text: &str, origin: &str) -> Saved {
    let mut body = saved(text, None);
    body.sources = Vec::new();
    body.imported = Some(Box::new(Origin {
        noted: at(1),
        origin: origin.to_string(),
    }));
    body
}

#[test]
fn noted_and_origin_round_trip_and_are_left_out_when_there_are_none() {
    let body = imported("用户养猫", "legacy:default:facts:7");
    let event =
        to_event(seq(3), at(100), owner(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    let line = event.to_line();
    assert!(
        line.ends_with(r#""noted":"2026-10-07T08:01:00.000Z","origin":"legacy:default:facts:7"}}"#),
        "{line}"
    );
    let back = Event::from_line(&line).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Saved(body))));
    let plain = to_event(
        seq(4),
        at(1),
        tool(),
        &MemoryEvent::Saved(saved("用户养猫", None)),
    )
    .expect("写得出");
    assert!(
        !plain.to_line().contains("noted") && !plain.to_line().contains("origin"),
        "没有的不写"
    );
    let old = Event::from_line(&plain.to_line()).expect("读得回");
    assert_eq!(
        from_event(&old),
        Some(Ok(MemoryEvent::Saved(saved("用户养猫", None)))),
        "以前的日志照读"
    );
}

#[test]
fn the_book_dates_an_import_by_when_it_was_noted() {
    let mut book = MemoryBook::default();
    see(
        &mut book,
        1,
        MemoryEvent::Saved(imported("用户养猫", "o:1")),
    );
    see(&mut book, 2, MemoryEvent::Saved(saved("用户住北京", None)));
    let first = book.get(MemoryId::new(seq(1))).expect("有");
    assert_eq!(first.at, at(1), "照原来记下的时刻");
    let second = book.get(MemoryId::new(seq(2))).expect("有");
    assert_eq!(second.at, at(100), "没有的照事件的时刻");
}

#[test]
fn an_origin_stays_imported_after_retiring_replacing_or_clearing() {
    let mut book = MemoryBook::default();
    assert!(!book.imported("o:1"));
    see(
        &mut book,
        1,
        MemoryEvent::Saved(imported("用户养猫", "o:1")),
    );
    see(
        &mut book,
        2,
        MemoryEvent::Saved(imported("用户住北京", "o:2")),
    );
    see(
        &mut book,
        3,
        MemoryEvent::Saved(imported("用户喝茶", "o:3")),
    );
    see(
        &mut book,
        4,
        MemoryEvent::Retired(Retired {
            id: MemoryId::new(seq(1)),
            why: String::new(),
            kind: None,
            kept: None,
        }),
    );
    see(
        &mut book,
        5,
        MemoryEvent::Saved(saved("用户住上海", Some(MemoryId::new(seq(2))))),
    );
    see(
        &mut book,
        6,
        MemoryEvent::Cleared(Cleared { session: None }),
    );
    for origin in ["o:1", "o:2", "o:3"] {
        assert!(book.imported(origin), "{origin}");
    }
    assert!(!book.imported("o:4"));
}
