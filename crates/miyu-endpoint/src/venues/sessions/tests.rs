//! 桥碰得到哪些场所会话（施工 Y-2 上；SU-4 中起只照平台认）：场所编号的平台前缀在它清单写的平台里的才碰得到；别的平台的、
//! 本机的、没有前缀的都碰不到。

use std::collections::BTreeSet;

use super::*;

#[test]
fn a_bridge_reaches_only_venues_on_its_platforms() {
    let platforms = Platforms(BTreeSet::from(["qq".to_string()]));
    assert!(platforms.holds("qq:group:1"));
    assert!(platforms.holds("qq:private:10001"));
    assert!(!platforms.holds("tg:group:1"), "别的平台的");
    assert!(!platforms.holds(LOCAL), "本机的");
    assert!(!platforms.holds("plain"), "没有前缀的");
    assert!(
        !Platforms(BTreeSet::new()).holds("qq:group:1"),
        "没写平台的"
    );
}
