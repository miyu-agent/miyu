//! 带的东西在转发里的编号（施工 O-61）：没有编号的照以前写，旧事件读进来再写出去一字不差；有的写成正整数，0 读不进来。

use super::*;

#[test]
fn media_without_an_index_reads_and_writes_as_before() {
    let old = r#"{"kind":"image","id":"i-1","name":"a.png","size":2048}"#;
    let media: Media = serde_json::from_str(old).expect("读得进来");
    assert_eq!(media.index, None);
    assert_eq!(serde_json::to_string(&media).expect("写得出"), old);
}

#[test]
fn an_index_is_a_positive_number_written_as_is() {
    let numbered = r#"{"kind":"image","id":"i-2","index":3}"#;
    let media: Media = serde_json::from_str(numbered).expect("读得进来");
    assert_eq!(media.index.map(std::num::NonZeroU32::get), Some(3));
    assert_eq!(serde_json::to_string(&media).expect("写得出"), numbered);
    assert!(serde_json::from_str::<Media>(r#"{"kind":"image","id":"i-3","index":0}"#).is_err());
}
