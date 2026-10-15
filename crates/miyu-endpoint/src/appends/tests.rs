//! 各个包放的（施工 X-20）：单给会话的优先、去掉了退回全部的那一份，全部的去掉了交 `None`，没说过话的包不在里面。

use super::Appends;

#[test]
fn a_session_one_wins_and_removing_falls_back() {
    let appends = Appends::default();
    assert!(appends.of("s1").is_empty(), "没说过话的不在里面");
    appends.set("memes", None, Some("all".to_string()));
    appends.set("memes", Some("s1".to_string()), Some("one".to_string()));
    assert_eq!(
        appends.of("s1")["memes"].as_deref(),
        Some("one"),
        "单给会话的优先"
    );
    assert_eq!(appends.of("s2")["memes"].as_deref(), Some("all"));
    appends.set("memes", Some("s1".to_string()), None);
    assert_eq!(
        appends.of("s1")["memes"].as_deref(),
        Some("all"),
        "去掉了退回全部的"
    );
    appends.set("memes", None, None);
    assert_eq!(appends.of("s1")["memes"], None, "全部的去掉了是 None");
}
