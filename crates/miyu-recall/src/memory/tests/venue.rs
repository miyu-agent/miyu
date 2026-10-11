//! 在哪个场所说的（施工 R-13 上，`memory.md`「对外的样子」）：场所里记的带 `venue`，写成事件读回一字不差；以前记的没有这一格，
//! 照样读得进；底账照记，改它的一条接着带。

use miyu_kernel::id::VenueId;

use super::*;

fn group() -> VenueId {
    VenueId::parse("qq:group:1").expect("合写法")
}

#[test]
fn a_memory_said_in_a_venue_round_trips_and_older_ones_have_none() {
    let mut body = saved("小林养了一只猫", None);
    body.audience = Vec::new();
    body.venue = Some(group());
    let event = to_event(seq(3), at(1), tool(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    assert!(
        event
            .to_line()
            .ends_with(r#""audience":[],"venue":"qq:group:1"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Saved(body))));

    let older = to_event(
        seq(4),
        at(1),
        tool(),
        &MemoryEvent::Saved(saved("用 N 卡", None)),
    )
    .expect("写得出");
    assert!(!older.to_line().contains("venue"), "没有的不写");

    let mut book = MemoryBook::default();
    book.see(&event).expect("读得懂");
    book.see(&older).expect("读得懂");
    assert_eq!(
        book.get(MemoryId::new(seq(3))).expect("在").venue,
        Some(group())
    );
    assert_eq!(book.get(MemoryId::new(seq(4))).expect("在").venue, None);
}
