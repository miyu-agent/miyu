//! 关键词（施工 R-15 上，`memory.md` 第二条）：`ext.memory.saved` 的 `keywords` 读写得回、没有的不写（以前的日志照读）；
//! `ext.memory.tagged` 读写得回，底账照它换这一条的关键词、编号不变，空的是去掉，摘要照旧算数；进检索库的字是正文接关键词；
//! 她给的关键词照 [`keywords_of`] 整理，太长的在逗号处截断。

use super::*;

fn see(book: &mut MemoryBook, n: u64, event: MemoryEvent) {
    let event = to_event(seq(n), at(1), owner(), &event).expect("写得出");
    book.see(&event).expect("读得懂");
}

fn m(n: u64) -> MemoryId {
    MemoryId::new(seq(n))
}

fn tagged(n: u64, keywords: &str) -> MemoryEvent {
    MemoryEvent::Tagged(Tagged {
        id: m(n),
        keywords: keywords.to_string(),
    })
}

#[test]
fn keywords_round_trip_and_are_left_out_when_there_are_none() {
    let mut body = saved("用 N 卡", None);
    body.keywords = Some("显卡,NVIDIA,GPU".to_string());
    let event = to_event(seq(7), at(1), tool(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    assert!(
        event
            .to_line()
            .ends_with(r#""audience":[{"kind":"person"}],"keywords":"显卡,NVIDIA,GPU"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Saved(body))));
    let plain = to_event(
        seq(8),
        at(1),
        tool(),
        &MemoryEvent::Saved(saved("用 N 卡", None)),
    )
    .expect("写得出");
    assert!(!plain.to_line().contains("keywords"), "没有的不写");
}

#[test]
fn a_tagging_round_trips() {
    let event = to_event(seq(5), at(1), owner(), &tagged(3, "猫,宠物")).expect("写得出");
    assert_eq!(event.body.kind(), "ext.memory.tagged");
    assert!(
        event
            .to_line()
            .ends_with(r#""body":{"id":"m3","keywords":"猫,宠物"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(tagged(3, "猫,宠物"))));
}

#[test]
fn the_book_changes_only_the_keywords_and_keeps_the_summary() {
    let mut book = MemoryBook::default();
    see(
        &mut book,
        1,
        MemoryEvent::Saved(saved("用户养了一只猫", None)),
    );
    see(
        &mut book,
        2,
        MemoryEvent::Summary(Summary {
            text: "养猫的人".into(),
            upto: seq(1),
        }),
    );
    see(&mut book, 3, tagged(1, "宠物,团子"));
    let entry = book.get(m(1)).expect("有");
    assert_eq!(entry.keywords.as_deref(), Some("宠物,团子"));
    assert!(entry.current(), "编号不变、照旧算数");
    assert!(
        book.summary().is_some() && !book.summary_stale(),
        "关键词不进摘要：摘要照旧算数"
    );
    see(&mut book, 4, tagged(1, ""));
    assert_eq!(book.get(m(1)).expect("有").keywords, None, "空的是去掉");
    see(&mut book, 5, tagged(9, "没有这一条"));
    assert!(book.get(m(9)).is_none(), "没有的那一条不碍事");
}

#[test]
fn the_indexed_text_is_the_text_and_then_the_keywords() {
    assert_eq!(
        indexed("用户养了一只猫", Some("宠物,团子")),
        "用户养了一只猫\n宠物,团子"
    );
    assert_eq!(indexed("用户养了一只猫", None), "用户养了一只猫");
    assert_eq!(indexed("用户养了一只猫", Some("")), "用户养了一只猫");
}

#[test]
fn keywords_are_trimmed_and_cut_at_a_comma() {
    assert_eq!(keywords_of("  猫,宠物  ").as_deref(), Some("猫,宠物"));
    assert_eq!(keywords_of("   "), None);
    let long = format!("{},尾巴", "猫".repeat(KEYWORD_CHARS - 2));
    assert_eq!(
        keywords_of(&long).as_deref(),
        Some("猫".repeat(KEYWORD_CHARS - 2).as_str()),
        "超长的在限度以内最后一个逗号处截断"
    );
    let chinese = format!("{}，尾巴", "猫".repeat(KEYWORD_CHARS - 2));
    assert_eq!(
        keywords_of(&chinese).map(|cut| cut.chars().count()),
        Some(KEYWORD_CHARS - 2),
        "全角逗号也认"
    );
    let flat = "猫".repeat(KEYWORD_CHARS + 5);
    assert_eq!(
        keywords_of(&flat).map(|cut| cut.chars().count()),
        Some(KEYWORD_CHARS),
        "一个逗号都没有的照字数截"
    );
    let exact = "猫".repeat(KEYWORD_CHARS);
    assert_eq!(
        keywords_of(&exact).as_deref(),
        Some(exact.as_str()),
        "正好的不动"
    );
}
