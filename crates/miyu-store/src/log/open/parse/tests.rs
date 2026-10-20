//! 分块读和一行行读交回的一样：照先后、行号对、读不成的停在第一处。

use super::{SPLIT_AT, cuts, lines};

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

#[test]
fn many_lines_come_back_in_order() {
    let texts = many();
    let parsed = lines(&joined(&texts));
    if let Some((n, why)) = &parsed.broken {
        panic!("第 {n} 行：{why}");
    }
    assert_eq!(parsed.events.len(), texts.len());
    for (k, event) in parsed.events.iter().enumerate() {
        assert_eq!(event.seq.get(), k as u64 + 1);
    }
    assert_eq!(parsed.events.capacity(), texts.len(), "一次留够，不多要");
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
    let parsed = lines(&joined(&texts));
    assert_eq!(parsed.events.len(), broken - 1, "第一处读不成的后面的不交");
    let (number, why) = parsed.broken.expect("读不成");
    assert_eq!(number, broken);
    assert!(why.starts_with("not readable"), "{why}");
}
