//! 分块读和一行行读交回的一样：照先后、行号对、读不成的停在第一处。

use miyu_kernel::event::Event;

use super::{Flow, SPLIT_AT, chunk, cuts, stream};

/// 第 `seq` 条的一行原文：alice 说的一句（同 `log/tests.rs` 的写法）。
fn line(seq: u64) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"alice"}},"body":{{"blocks":[{{"type":"text","text":"第 {seq} 句"}}]}}}}"#
    )
}

/// 行多到分得成几块：一行两百字节上下，够 [`SPLIT_AT`] 的三倍。
fn many() -> Vec<String> {
    (1..=(SPLIT_AT / 200 * 3) as u64).map(line).collect()
}

/// 每行接上 `\n`。
fn joined(texts: &[String]) -> Vec<u8> {
    texts
        .iter()
        .flat_map(|text| format!("{text}\n").into_bytes())
        .collect()
}

/// 一块块收齐：交出去的事件、读不成的那一行。
fn streamed(bytes: &[u8]) -> (Vec<Event>, Option<(usize, String)>) {
    let mut got = Vec::new();
    let broken = stream(bytes, |events| {
        got.extend(events);
        Flow::Go
    });
    (got, broken)
}

#[test]
fn many_lines_come_back_in_order() {
    let texts = many();
    let (events, broken) = streamed(&joined(&texts));
    if let Some((n, why)) = &broken {
        panic!("第 {n} 行：{why}");
    }
    assert_eq!(events.len(), texts.len());
    for (k, event) in events.iter().enumerate() {
        assert_eq!(event.seq.get(), k as u64 + 1);
    }
}

#[test]
fn every_cut_falls_at_the_start_of_a_line() {
    let bytes = joined(&many());
    let cut = cuts(&bytes, 7);
    assert!(cut.len() > 2, "分成了几块：{cut:?}");
    assert_eq!(cut.first(), Some(&0));
    assert_eq!(cut.last(), Some(&bytes.len()));
    for at in &cut[1..cut.len() - 1] {
        assert_eq!(bytes[at - 1], b'\n', "第 {at} 字节前面是换行");
    }
    assert!(cut.windows(2).all(|pair| pair[0] < pair[1]), "{cut:?}");
    // 只有一行、很长的：分不开，一块。
    let one = joined(&[line(1)]);
    assert_eq!(cuts(&one, 4), vec![0, one.len()]);
}

#[test]
fn reading_stops_at_the_first_broken_line() {
    let mut texts = many();
    let broken = texts.len() * 2 / 3 + 7;
    texts[broken - 1] = "{not json".to_string();
    texts[broken + 100] = "{also not json".to_string();
    let (events, bad) = streamed(&joined(&texts));
    assert_eq!(events.len(), broken - 1, "第一处读不成的后面的不交");
    let (number, why) = bad.expect("读不成");
    assert_eq!(number, broken);
    assert!(why.starts_with("not readable"), "{why}");
}

/// 一块块交（施工 V-15）：照先后交，接起来和一个线程一行行读的一样，分了不止一块。
#[test]
fn streamed_pieces_join_up_in_order() {
    let bytes = joined(&many());
    let whole = chunk(&bytes).events;
    let mut got = Vec::new();
    let mut pieces = 0;
    let broken = stream(&bytes, |events| {
        pieces += 1;
        got.extend(events);
        Flow::Go
    });
    assert!(broken.is_none());
    assert!(pieces > 1, "分成了几块交：{pieces}");
    assert_eq!(got, whole);
}

/// 读不成的那一行照整段数，它前面的照样交，后面的不交（同 [`lines`]）。
#[test]
fn a_streamed_broken_line_counts_from_the_segment_start() {
    let mut texts = many();
    let broken = texts.len() * 2 / 3 + 7;
    texts[broken - 1] = "{not json".to_string();
    let bytes = joined(&texts);
    let mut got = 0;
    let bad = stream(&bytes, |events| {
        got += events.len();
        Flow::Go
    });
    assert_eq!(bad.map(|(number, _)| number), Some(broken));
    assert_eq!(got, broken - 1);
}

/// 叫停了就不再交，也不报后面读不成的。
#[test]
fn a_stopped_stream_hands_over_nothing_more() {
    let mut texts = many();
    let last = texts.len() - 1;
    texts[last] = "{not json".to_string();
    let bytes = joined(&texts);
    let mut calls = 0;
    let bad = stream(&bytes, |_| {
        calls += 1;
        Flow::Stop
    });
    assert_eq!(calls, 1);
    assert!(bad.is_none());
    // 小的不分块，一样照 `Flow` 走。
    let small = joined(&[line(1), line(2)]);
    let mut seen = Vec::new();
    assert!(
        stream(&small, |events| {
            seen.extend(events.iter().map(|event| event.seq.get()));
            Flow::Go
        })
        .is_none()
    );
    assert_eq!(seen, [1, 2]);
}
