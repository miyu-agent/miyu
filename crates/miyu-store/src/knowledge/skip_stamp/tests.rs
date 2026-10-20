//! 跳过的指纹：写了读得回来；文件变了的、装上了抽字的小程序的、正常的指纹不算。

use super::*;

#[test]
fn a_skip_is_read_back_while_the_file_stays_the_same() {
    let recorded = skipped("12:34", Reason::NotUtf8, "not valid UTF-8");
    assert_eq!(
        read(&recorded, "12:34", false),
        Some((Reason::NotUtf8, "not valid UTF-8".to_string()))
    );
    assert_eq!(read(&recorded, "13:34", false), None, "文件变了的重读");
    assert_eq!(read("12:34", "12:34", false), None, "正常的指纹不是跳过");
}

#[test]
fn no_extractor_counts_only_while_there_is_none() {
    let recorded = skipped("1:2", Reason::NoExtractor, "no extractor");
    assert!(read(&recorded, "1:2", false).is_some());
    assert_eq!(read(&recorded, "1:2", true), None, "装上了就重读");
    let failed = skipped("1:2", Reason::ExtractFailed, "timed out");
    assert!(
        read(&failed, "1:2", true).is_some(),
        "出错的照记下的，重建索引时再试"
    );
}

#[test]
fn a_skipped_document_counts_only_while_its_old_chunks_stay() {
    let stamps = BTreeMap::from([
        ("ok.md".to_string(), "1:1".to_string()),
        (
            "kept.md".to_string(),
            skipped("2:2", Reason::NotUtf8, "bad"),
        ),
        (
            "never.txt".to_string(),
            skipped("3:3", Reason::TooLarge, "big"),
        ),
    ]);
    let keys = [
        "ok.md#0".to_string(),
        "kept.md#0".to_string(),
        "kept.md#1".to_string(),
    ];
    assert_eq!(indexed(&stamps, &keys), 2);
}
