//! 人卡（施工 E-1 下）：读写得回、没写的叫法不写；底账一个人照最新的那一张，作废了的记着为什么、再写一张又算数；整间清空拿掉，
//! 只清一个会话的不动。

use super::*;

fn card(who: &str, text: &str, names: &[&str]) -> MemoryEvent {
    MemoryEvent::Card(Card {
        who: who.to_string(),
        text: text.to_string(),
        names: names.iter().map(ToString::to_string).collect(),
    })
}

#[test]
fn a_card_round_trips_and_the_book_keeps_the_latest_per_person() {
    let first = to_event(
        seq(1),
        at(1),
        admin(),
        &card("qq:10086", "在 Rust 群认识的。", &[]),
    )
    .expect("写得出");
    assert!(
        first
            .to_line()
            .ends_with(r#""body":{"who":"qq:10086","text":"在 Rust 群认识的。"}}"#),
        "没有叫法的不写：{}",
        first.to_line()
    );
    let newer = card(
        "qq:10086",
        "在 Rust 群认识的，用 RX 7800 XT。",
        &["GNU/Rust++"],
    );
    let second = to_event(seq(2), at(2), admin(), &newer).expect("写得出");
    let other = to_event(seq(3), at(3), admin(), &card("小林", "大学同学。", &[])).expect("写得出");
    let mut book = MemoryBook::default();
    for event in [&first, &second, &other] {
        let back = Event::from_line(&event.to_line()).expect("读得回");
        assert!(from_event(&back).is_some_and(|read| read.is_ok()));
        book.see(&back).expect("读得懂");
    }
    let latest = book.card("qq:10086").expect("有他的卡");
    assert_eq!(latest.text, "在 Rust 群认识的，用 RX 7800 XT。");
    assert_eq!(latest.names, vec!["GNU/Rust++".to_string()]);
    assert_eq!(latest.at, at(2));
    assert_eq!(
        book.cards()
            .map(|card| card.who.as_str())
            .collect::<Vec<_>>(),
        vec!["qq:10086", "小林"],
        "一个人一张"
    );
}

#[test]
fn a_retired_card_says_why_and_a_new_one_counts_again() {
    let retired = MemoryEvent::CardRetired(CardRetired {
        who: "qq:10086".into(),
        why: "认错人了".into(),
    });
    let mut book = MemoryBook::default();
    for (n, event) in [card("qq:10086", "在 Rust 群认识的。", &[]), retired.clone()]
        .iter()
        .enumerate()
    {
        let line = to_event(seq(n as u64 + 1), at(1), admin(), event)
            .expect("写得出")
            .to_line();
        book.see(&Event::from_line(&line).expect("读得回"))
            .expect("读得懂");
    }
    assert_eq!(
        book.card("qq:10086").and_then(|card| card.retired.clone()),
        Some("认错人了".to_string())
    );
    let again = to_event(
        seq(3),
        at(2),
        admin(),
        &card("qq:10086", "重新认识了。", &[]),
    )
    .expect("写得出");
    book.see(&again).expect("读得懂");
    assert_eq!(
        book.card("qq:10086").and_then(|card| card.retired.clone()),
        None
    );
}

#[test]
fn clearing_everything_drops_the_cards_but_clearing_a_session_does_not() {
    let mut book = MemoryBook::default();
    book.see(&to_event(seq(1), at(1), admin(), &card("小林", "大学同学。", &[])).expect("写得出"))
        .expect("读得懂");
    let session = MemoryEvent::Cleared(Cleared {
        session: Some(source(3).session),
    });
    book.see(&to_event(seq(2), at(2), admin(), &session).expect("写得出"))
        .expect("读得懂");
    assert!(book.card("小林").is_some(), "只清一个会话的不动卡");
    let all = MemoryEvent::Cleared(Cleared { session: None });
    book.see(&to_event(seq(3), at(3), admin(), &all).expect("写得出"))
        .expect("读得懂");
    assert!(book.card("小林").is_none(), "整间清空一起拿掉");
}
