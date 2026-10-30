//! 支线替她回的话（施工 E-8）：群主线抽取那一段照先后排进别的线替她发进群里的话，写成群聊近况的 `[you]` 行。

use miyu_store::log::SessionLog;

use super::{BRANCH, Fixture, Log, MAIN};
use crate::memory::Plan;

/// 支线替她回的话（施工 E-8）：照先后排进那一轮人这边的几句里，写成群聊近况的 `[you]` 行；这条线自己发的不排（她的回答已经在
/// 里面）；`you` 不当成一个人记名字。
#[test]
fn what_a_branch_said_for_her_comes_in_as_her_line() {
    let fixture = Fixture::new("branch");
    let mut log = SessionLog::create(&fixture.dir, 1 << 20).expect("建得了日志");
    let mut events = Log::new();
    events.said(10_001, "小林", "米羽在吗", None, None);
    events.delivered(BRANCH, "在的，怎么啦");
    events.said(10_001, "小林", "我下周去大阪", None, None);
    events.delivered(MAIN, "主线自己发的那句");
    events.answer("主线自己发的那句", false);
    log.append(&events.take()).expect("写得进");
    let Plan::Ask { text, names, .. } = fixture.whole(1) else {
        panic!("抽到了东西");
    };
    let asked = text.find("米羽在吗").expect("人话在");
    let branch = text
        .find("] [you] [msg=d3]: 在的，怎么啦")
        .unwrap_or_else(|| panic!("支线回的那句排进来了：{text}"));
    let later = text.find("我下周去大阪").expect("人话在");
    assert!(asked < branch && branch < later, "照先后：{text}");
    assert_eq!(
        text.matches("主线自己发的那句").count(),
        1,
        "这条线自己发的只作她的回答出现一次：{text}"
    );
    assert!(!text.contains("[msg=d5]"), "{text}");
    assert!(
        !names.keys().any(|name| name.eq_ignore_ascii_case("you")),
        "{names:?}"
    );
}
