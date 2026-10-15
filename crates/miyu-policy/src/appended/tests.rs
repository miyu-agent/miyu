//! 扩展放在系统提示词尾部的那几段（施工 X-20）：照包的编号排、接在最后、空一行，记进快照；没有的一字不变。

use std::collections::BTreeMap;

use crate::test_support::engineer;

#[test]
fn appended_passages_go_last_in_package_order() {
    let snapshot = engineer().with_style_lock("Stay in character.");
    let appended = BTreeMap::from([
        (
            "onebot".to_string(),
            "<group-style>Be brief.</group-style>".to_string(),
        ),
        (
            "memes".to_string(),
            "<memes>A meme ends your turn.</memes>".to_string(),
        ),
    ]);
    let with = snapshot.clone().with_appended(appended.clone());
    assert_eq!(
        with.system,
        format!(
            "{}\n\n<memes>A meme ends your turn.</memes>\n\n<group-style>Be brief.</group-style>",
            snapshot.system
        ),
        "照编号排，接在风格锁后面"
    );
    assert_eq!(with.appended, appended);
    assert_eq!(
        snapshot.clone().with_appended(BTreeMap::new()),
        snapshot,
        "没有的一字不变"
    );
}
