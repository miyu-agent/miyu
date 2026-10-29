//! 回合库里哪个会话在哪个场所（施工 R-13 下，`docs/blueprint/recall.md`「库的结构」、`memory.md` 第一条第 1 款）：记一次、再记
//! 换成新的；照来源列出；删会话（`forget`）跟着拿掉；重开还在，重建是空的。每个会话从哪来（`origins`，施工 E-6）同样。

use std::path::PathBuf;

use miyu_store::recall::{Origin, RecallIndex};

use crate::support::*;

fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index()
        .join("recall")
        .join("turns-places.db")
}

#[test]
fn a_session_is_placed_once_and_goes_with_its_source() {
    let scratch = Scratch::new("recall-places");
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert!(index.places().expect("读得了").is_empty());
    index.place("s1", "qq:group:1").expect("写得进");
    index.place("s2", "qq:group:2").expect("写得进");
    index.place("s2", "qq:group:2").expect("再记一次不碍事");
    let places = index.places().expect("读得了");
    assert_eq!(places.get("s1").map(String::as_str), Some("qq:group:1"));
    assert_eq!(places.get("s2").map(String::as_str), Some("qq:group:2"));
    assert_eq!(places.len(), 2);

    index.forget("s1").expect("拿得掉");
    assert!(
        !index.places().expect("读得了").contains_key("s1"),
        "删会话跟着拿掉"
    );

    drop(index);
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert_eq!(index.places().expect("读得了").len(), 1, "重开还在");
    index.reset().expect("建得成");
    assert!(index.places().expect("读得了").is_empty(), "重建是空的");
}

/// 每个会话从哪来（施工 E-6）：属主、是不是主会话，记一次、再记换成新的；删会话跟着拿掉；重开还在，重建是空的。
#[test]
fn where_a_session_came_from_is_kept_with_its_source() {
    let scratch = Scratch::new("recall-origins");
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert!(index.origins().expect("读得了").is_empty());
    let origin = |main: bool| Origin { main };
    index.origin("s1", &origin(true)).expect("写得进");
    index.origin("s2", &origin(true)).expect("写得进");
    index.origin("s2", &origin(false)).expect("再记换成新的");
    let origins = index.origins().expect("读得了");
    assert_eq!(origins.get("s1"), Some(&origin(true)));
    assert_eq!(origins.get("s2"), Some(&origin(false)));
    assert_eq!(origins.len(), 2);

    index.forget("s1").expect("拿得掉");
    assert!(
        !index.origins().expect("读得了").contains_key("s1"),
        "删会话跟着拿掉"
    );

    drop(index);
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert_eq!(index.origins().expect("读得了").len(), 1, "重开还在");
    index.reset().expect("建得成");
    assert!(index.origins().expect("读得了").is_empty(), "重建是空的");
}
