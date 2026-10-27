//! 在哪个群（施工 E-4 上，`memory.md`「对外的样子」）：群里记的带 `place`（记下那一刻的群名），写成事件读回一字不差；以前记的
//! 没有这一格，照样读得进、不写；底账照记。群名是编的。

use super::*;

#[test]
fn a_memory_with_a_group_name_round_trips_and_older_ones_have_none() {
    let mut body = saved("小林养了一只猫", None);
    body.place = Some("周末摄影群".to_string());
    let event = to_event(seq(3), at(1), tool(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    assert!(
        event.to_line().ends_with(r#","place":"周末摄影群"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Saved(body))));

    // 以前记的：日志里没有这一格。
    let older = r#"{"seq":4,"at":"2026-10-07T08:01:00.000Z","kind":"ext.memory.saved","by":{"kind":"tool","call_id":"call_3_1"},"body":{"class":"user","text":"用 N 卡","sources":[],"audience":[]}}"#;
    let older = Event::from_line(older).expect("读得回");
    let Some(Ok(MemoryEvent::Saved(read))) = from_event(&older) else {
        panic!("读得懂");
    };
    assert_eq!(read.place, None);
    let bare = to_event(
        seq(5),
        at(1),
        tool(),
        &MemoryEvent::Saved(saved("用 N 卡", None)),
    )
    .expect("写得出");
    assert!(!bare.to_line().contains("place"), "没有的不写");

    let mut book = MemoryBook::default();
    book.see(&event).expect("读得懂");
    book.see(&older).expect("读得懂");
    assert_eq!(
        book.get(MemoryId::new(seq(3)))
            .expect("在")
            .place
            .as_deref(),
        Some("周末摄影群")
    );
    assert_eq!(book.get(MemoryId::new(seq(4))).expect("在").place, None);
}
