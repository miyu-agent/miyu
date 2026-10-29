//! 整理留痕（施工 R-17，`memory.md` 第七条第 10 款）：作废的种类、留下的那一条，开始合一批、没合成的读写得回；底账记着开始合了
//! 还没收尾的那一批，同一个 `upto` 的 `merged`、`merge_failed` 才收尾。

use super::*;

fn line(seq_no: u64, event: &MemoryEvent) -> String {
    to_event(seq(seq_no), at(1), admin(), event)
        .expect("写得出")
        .to_line()
}

fn round_trip(event: &MemoryEvent) {
    let back = Event::from_line(&line(9, event)).expect("读得回");
    assert_eq!(from_event(&back).as_ref(), Some(&Ok(event.clone())));
}

#[test]
fn a_retirement_by_the_organizer_says_its_kind_and_what_was_kept() {
    let outdated = MemoryEvent::Retired(Retired {
        id: MemoryId::new(seq(7)),
        why: "搬家了".into(),
        kind: Some(RetireKind::Outdated),
        kept: Some(MemoryId::new(seq(9))),
    });
    assert!(
        line(10, &outdated)
            .ends_with(r#""body":{"id":"m7","why":"搬家了","kind":"outdated","kept":"m9"}}"#),
        "{}",
        line(10, &outdated)
    );
    round_trip(&outdated);
    let worthless = MemoryEvent::Retired(Retired {
        id: MemoryId::new(seq(7)),
        why: "一次性的".into(),
        kind: Some(RetireKind::Worthless),
        kept: None,
    });
    assert!(line(10, &worthless).ends_with(r#""kind":"worthless"}}"#));
    round_trip(&worthless);
    let wrong = Event::from_line(
        r#"{"seq":1,"at":"2026-10-07T08:00:00.000Z","kind":"ext.memory.retired","by":{"kind":"kernel"},"body":{"id":"m1","why":"x","kind":"old"}}"#,
    )
    .expect("外壳读得出");
    assert!(
        matches!(from_event(&wrong), Some(Err(_))),
        "种类写错的读不懂"
    );
}

#[test]
fn starting_a_batch_and_failing_it_round_trip() {
    let merging = MemoryEvent::Merging(Merging {
        upto: seq(4),
        fresh: 2,
        left: 5,
        manual: true,
        given: vec![MemoryId::new(seq(1)), MemoryId::new(seq(4))],
        session: None,
    });
    assert!(
        line(6, &merging).contains(
            r#""kind":"ext.memory.merging","by":{"kind":"person"},"body":{"upto":4,"fresh":2,"left":5,"manual":true,"given":["m1","m4"]}}"#
        ),
        "{}",
        line(6, &merging)
    );
    round_trip(&merging);
    let failed = MemoryEvent::MergeFailed(MergeFailed {
        upto: seq(4),
        why: "no JSON object in the answer".into(),
        tries: Some(2),
    });
    assert!(line(7, &failed).ends_with(
        r#""kind":"ext.memory.merge_failed","by":{"kind":"person"},"body":{"upto":4,"why":"no JSON object in the answer","tries":2}}"#
    ));
    round_trip(&failed);
    let interrupted = MemoryEvent::MergeFailed(MergeFailed {
        upto: seq(4),
        why: INTERRUPTED.into(),
        tries: None,
    });
    assert!(
        line(8, &interrupted).ends_with(r#""body":{"upto":4,"why":"interrupted"}}"#),
        "半路停了的不写第几次：{}",
        line(8, &interrupted)
    );
    round_trip(&interrupted);
}

#[test]
fn the_book_knows_the_batch_that_has_not_ended() {
    let mut book = MemoryBook::default();
    let mut next = 0;
    let mut see = |book: &mut MemoryBook, event: MemoryEvent| {
        next += 1;
        book.see(&to_event(seq(next), at(1), admin(), &event).expect("写得出"))
            .expect("读得懂");
    };
    let merging = |upto| {
        MemoryEvent::Merging(Merging {
            upto: seq(upto),
            fresh: 1,
            left: 1,
            manual: false,
            given: Vec::new(),
            session: None,
        })
    };
    let failed = |upto| {
        MemoryEvent::MergeFailed(MergeFailed {
            upto: seq(upto),
            why: "x".into(),
            tries: Some(1),
        })
    };
    assert_eq!(book.merging(), None);
    see(&mut book, merging(3));
    assert_eq!(book.merging(), Some(seq(3)));
    see(&mut book, failed(2));
    assert_eq!(book.merging(), Some(seq(3)), "别的一批的不算收尾");
    see(
        &mut book,
        MemoryEvent::Merged(Merged {
            upto: seq(3),
            given: 1,
            revised: 0,
            retired: 0,
            failed: false,
        }),
    );
    assert_eq!(book.merging(), None, "合完了");
    see(&mut book, merging(5));
    see(&mut book, failed(5));
    assert_eq!(book.merging(), None, "没合成也是收尾");
}

/// 跑在整理会话里的一批带 `session`（施工 D-2），读回来照旧；没有的不写这一格。
#[test]
fn a_batch_in_a_dream_session_names_it() {
    let session = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").expect("合写法");
    let merging = MemoryEvent::Merging(Merging {
        upto: seq(4),
        fresh: 1,
        left: 1,
        manual: false,
        given: Vec::new(),
        session: Some(session.clone()),
    });
    let written = line(6, &merging);
    assert!(
        written.contains(&format!(r#""session":"{session}""#)),
        "{written}"
    );
    round_trip(&merging);
}
