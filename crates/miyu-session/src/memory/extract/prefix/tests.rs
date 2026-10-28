//! 折到哪、记下的几处（施工 E-5 补）：接不上的时候不记、不挑，等整份读一次再接。名字、群号都是编的。

use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;
use miyu_store::log::Mark;

use super::*;

fn line(seq: u64, kind: &str, body: &str, turn: Option<u64>) -> Event {
    let turn = turn
        .map(|turn| format!(r#""turn":{turn},"#))
        .unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-28T03:00:00.000Z","kind":"{kind}",{turn}"by":{{"kind":"kernel"}},"cause":"c","body":{body}}}"#
    ))
    .expect("合写法")
}

fn said(seq: u64, id: &str, name: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-28T03:00:00.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5","id":"{id}"}},"body":{{"blocks":[{{"type":"text","text":"话"}}],"venue":{{"msg":"{seq}","name":"{name}","place":"群{seq}"}}}}}}"#
    ))
    .expect("合写法")
}

/// 一轮：人说一句、回合开始、结束。
fn round(from: u64) -> Vec<Event> {
    vec![
        said(from, "qq:2", "小林"),
        line(
            from + 1,
            "turn.started",
            &format!(r#"{{"trigger":{from}}}"#),
            Some(from + 1),
        ),
        line(
            from + 2,
            "turn.ended",
            r#"{"reason":"completed"}"#,
            Some(from + 1),
        ),
    ]
}

fn mark(next: u64) -> Mark {
    Mark {
        segment: 1,
        bytes: next * 100,
        next: Seq::new(next).expect("从 1 起"),
    }
}

fn seeded() -> Marks {
    let mut marks = Marks::default();
    let first = round(1);
    marks.heard(&first, true);
    marks.seed(Landed {
        mark: mark(4),
        prefix: Prefix::of(&first),
    });
    marks
}

/// 接上以后落了有回合结束的一批记一处；那一处认出来的名字、群名是到那一批为止的。
#[test]
fn a_landing_is_kept_for_every_batch_that_ends_a_turn() {
    let mut marks = seeded();
    assert_eq!(marks.landings().landings.len(), 1, "接上时读到的末尾那一处");
    assert!(!marks.landings().seed);
    marks.heard(&[said(4, "qq:3", "阿杰")], true);
    marks.landed(mark(5));
    assert_eq!(marks.landings().landings.len(), 1, "没有回合结束的不记");
    marks.heard(&round(5), true);
    marks.landed(mark(8));
    let landings = marks.landings().landings;
    assert_eq!(landings.len(), 2);
    let last = &landings[1].prefix;
    assert_eq!(
        last.names.get("阿杰"),
        Some(&Some("qq:3".to_string())),
        "折进了中间那一批"
    );
    assert_eq!(
        last.place.as_ref().map(|(_, place)| place.as_str()),
        Some("群5")
    );
}

/// 落了撤销（恢复、撤回同）的：认出来的名字和整份读的对不上了，不再记，一直整份读。
#[test]
fn an_undo_stops_landings_for_good() {
    let mut marks = seeded();
    marks.heard(&[line(4, "turn.reverted", r#"{"turns":[2]}"#, None)], true);
    marks.landed(mark(5));
    marks.heard(&round(5), true);
    marks.landed(mark(8));
    let landings = marks.landings();
    assert!(landings.landings.is_empty() && !landings.seed, "一直整份读");
    // 整份读回来认出的也不接。
    marks.seed(Landed {
        mark: mark(8),
        prefix: Prefix::of(&round(1)),
    });
    assert!(marks.landings().landings.is_empty());
}

/// 折到的和日志照到的对不上（有一批没经这里）：不知道了，等整份读一次再接上。
#[test]
fn a_landing_that_does_not_match_starts_over() {
    let mut marks = seeded();
    marks.heard(&round(4), true);
    marks.landed(mark(9));
    let landings = marks.landings();
    assert!(landings.landings.is_empty() && landings.seed);
}

/// 还接不上时记下的批：读到的末尾以后的折进去；记得太多从头记的、头一批比读到的末尾还晚的，接不上，等下次。
#[test]
fn a_whole_read_catches_up_only_when_nothing_was_missed() {
    let mut marks = Marks::default();
    let early = round(1);
    marks.heard(&early, true);
    let mut later = round(4);
    later.extend(round(7));
    marks.heard(&later, true);
    marks.seed(Landed {
        mark: mark(4),
        prefix: Prefix::of(&early),
    });
    let landings = marks.landings();
    assert!(!landings.seed, "接上了");
    let running = marks.running.as_ref().expect("接上了");
    assert_eq!(
        running.next,
        Some(Seq::new(10).expect("从 1 起")),
        "读完以后落的折进去了"
    );

    let mut marks = Marks::default();
    let mut seq = 1;
    while seq < 1100 {
        marks.heard(&round(seq), true);
        seq += 3;
    }
    marks.seed(Landed {
        mark: mark(4),
        prefix: Prefix::of(&round(1)),
    });
    assert!(marks.landings().seed, "记得太多从头记了，中间漏了，接不上");

    let mut marks = Marks::default();
    marks.heard(&round(1), false);
    assert!(marks.held.is_empty(), "不抽的会话不记");
}

/// 记下的几处有上限：旧的先丢。
#[test]
fn landings_are_bounded() {
    let mut marks = seeded();
    let mut seq = 4;
    for _ in 0..LANDINGS + 5 {
        marks.heard(&round(seq), true);
        seq += 3;
        marks.landed(mark(seq));
    }
    let landings = marks.landings().landings;
    assert_eq!(landings.len(), LANDINGS);
    assert_eq!(landings[LANDINGS - 1].mark.next.get(), seq);
}
