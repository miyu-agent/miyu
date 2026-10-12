//! 检索库里每个来源的指纹（施工 K-1 上，`docs/blueprint/recall.md`「库的结构」，`knowledge.md`「怎么走」第三条）：整份换掉
//! 一个来源（原来的块拿掉、新的放进去、指纹记下），别的来源不动；列出指纹；拿掉一个来源连指纹一起。

use std::path::PathBuf;

use miyu_store::recall::RecallIndex;

use crate::support::*;

fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch).index(&admin()).join("kb").join("notes.db")
}

fn keys(index: &RecallIndex) -> Vec<String> {
    index.keys().expect("读得了")
}

#[test]
fn a_source_is_replaced_whole_and_others_stay() {
    let scratch = Scratch::new("recall-stamps");
    let (index, _) = RecallIndex::open(&place(&scratch));
    let chunks =
        |texts: &[&str]| -> Vec<String> { texts.iter().map(ToString::to_string).collect() };
    index
        .replace("a.md", &chunks(&["猫叫团子", "团子三岁"]), at(1), "10:1")
        .expect("写得进");
    index
        .replace("b.md", &chunks(&["狗叫阿黄"]), at(1), "20:1")
        .expect("写得进");
    assert_eq!(keys(&index), ["a.md#0", "a.md#1", "b.md#0"]);
    index
        .replace("a.md", &chunks(&["猫叫小白"]), at(2), "11:2")
        .expect("写得进");
    assert_eq!(keys(&index), ["a.md#0", "b.md#0"], "原来的块整份拿掉");
    let found = index.search("小白", 10).expect("搜得了");
    assert_eq!(found.first().map(|hit| hit.key.as_str()), Some("a.md#0"));
    assert!(index.search("团子", 10).expect("搜得了").is_empty());
    let stamps = index.stamps().expect("读得了");
    assert_eq!(stamps.get("a.md").map(String::as_str), Some("11:2"));
    assert_eq!(stamps.get("b.md").map(String::as_str), Some("20:1"));

    index.drop_source("a.md").expect("拿得掉");
    assert_eq!(keys(&index), ["b.md#0"]);
    assert!(!index.stamps().expect("读得了").contains_key("a.md"));
    // `a.md` 是另一个来源 `a.md.bak` 的前缀也不碍事：照 `#` 分。
    index
        .replace("a.md.bak", &chunks(&["备份"]), at(3), "1:1")
        .expect("写得进");
    index.drop_source("a.md").expect("拿得掉");
    assert_eq!(keys(&index), ["a.md.bak#0", "b.md#0"]);
}

#[test]
fn replacing_or_dropping_a_source_drops_its_vectors() {
    // 施工 K-2：文档变了、没了，原来那几块的向量跟着拿掉（下一次搜完照新的补），别的来源的不动。
    let scratch = Scratch::new("recall-stamps-vectors");
    let (index, _) = RecallIndex::open(&place(&scratch));
    let one = |text: &str| vec![text.to_string()];
    index
        .replace("a.md", &one("猫叫团子"), at(1), "10:1")
        .expect("写得进");
    index
        .replace("b.md", &one("狗叫阿黄"), at(1), "20:1")
        .expect("写得进");
    for key in ["a.md#0", "b.md#0"] {
        index.put_vector(key, "m", &[1.0, 0.0]).expect("写得进");
    }
    let missing = |index: &RecallIndex| -> Vec<String> {
        index
            .missing("m", 0, 10)
            .expect("读得了")
            .into_iter()
            .map(|(_, key, _)| key)
            .collect()
    };
    assert!(missing(&index).is_empty());
    index
        .replace("a.md", &one("猫叫团子"), at(2), "10:2")
        .expect("写得进");
    assert_eq!(missing(&index), ["a.md#0"], "换掉的那份要重新补");
    index
        .put_vector("a.md#0", "m", &[1.0, 0.0])
        .expect("写得进");
    index.drop_source("a.md").expect("拿得掉");
    assert!(missing(&index).is_empty());
    assert_eq!(
        index.nearest("m", &[1.0, 0.0], 10).expect("找得了").len(),
        1,
        "只剩 b.md 的"
    );
}
