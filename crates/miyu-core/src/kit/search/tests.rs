//! 扩展回的几条怎么读（施工 N-5）：没有标题、网址的丢掉，空的摘录当没有，日期只留前十个字。

use super::*;

#[test]
fn hits_keep_the_written_ones() {
    let result = json!({"hits": [
        {"title": "Tokio", "url": "https://tokio.rs/", "snippet": " ", "date": "2026-10-19T08:00:00Z"},
        {"title": "", "url": "https://x.example/"},
        {"url": "https://y.example/"},
        {"title": "Rust", "url": "https://rust-lang.org/", "snippet": "A language"}
    ]});
    let got = hits(&result);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].snippet, None, "空的摘录当没有");
    assert_eq!(got[0].date.as_deref(), Some("2026-10-19"));
    assert_eq!(got[1].snippet.as_deref(), Some("A language"));
    assert!(hits(&json!({})).is_empty());
}
