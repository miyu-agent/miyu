//! 记内容、记谁在看（施工 X-15 中）：从没人看到有人看、从有人看到没人看各说一声，中间的不说；跟着会话的没人看的不留、
//! 最后一个人走了扔掉；程序停了全清。

use std::collections::BTreeSet;

use serde_json::json;

use miyu_kernel::id::AccountId;

use super::{Key, Shown, Widgets};
use crate::extensions::Instance;

fn key(widget: &str, session: Option<&str>) -> Key {
    Key {
        package: "wx".into(),
        widget: widget.into(),
        session: session.map(str::to_string),
        account: None,
    }
}

fn shown(text: &str) -> Shown {
    Shown {
        content: json!([{"type": "text", "text": text}]),
        title: None,
    }
}

#[test]
fn only_the_first_look_and_the_last_leave_are_told() {
    let widgets = Widgets::default();
    let mut told = widgets.watch_changes();
    let today: BTreeSet<Key> = [key("today", Some("s1"))].into();
    widgets.look(&today);
    widgets.look(&today);
    widgets.leave(&today);
    assert_eq!(
        widgets.watched(&Instance::system("wx")),
        today,
        "还有一个人看着"
    );
    widgets.leave(&today);
    assert!(widgets.watched(&Instance::system("wx")).is_empty());
    let first = told.try_recv().expect("开始看说一声");
    assert!(first.on && first.key == key("today", Some("s1")));
    let last = told.try_recv().expect("最后一个走了说一声");
    assert!(!last.on);
    assert!(told.try_recv().is_err(), "中间的不说");
    widgets.leave(&today);
    assert!(told.try_recv().is_err(), "没人看的再走不说");
}

#[test]
fn session_content_is_kept_only_while_someone_looks() {
    let widgets = Widgets::default();
    let mut changed = widgets.changes();
    assert!(
        !widgets.put(key("today", Some("s1")), shown("x")),
        "没人看的会话不留"
    );
    assert!(changed.try_recv().is_err());
    let today: BTreeSet<Key> = [key("today", Some("s1"))].into();
    widgets.look(&today);
    assert!(widgets.put(key("today", Some("s1")), shown("sunny")));
    assert_eq!(changed.try_recv().ok(), Some(key("today", Some("s1"))));
    assert_eq!(
        widgets
            .get(&key("today", Some("s1")))
            .map(|one| one.content),
        Some(json!([{"type": "text", "text": "sunny"}]))
    );
    widgets.leave(&today);
    assert!(
        widgets.get(&key("today", Some("s1"))).is_none(),
        "最后一个走了扔掉"
    );
    // 全局的不管有没有人看都留着。
    assert!(widgets.put(key("temp", None), shown("23°C")));
    assert!(widgets.get(&key("temp", None)).is_some());
}

#[test]
fn clear_and_forget_say_what_changed() {
    let widgets = Widgets::default();
    assert!(widgets.put(key("temp", None), shown("23°C")));
    assert!(widgets.put(key("week", None), shown("…")));
    let mut changed = widgets.changes();
    widgets.clear(key("temp", None));
    assert_eq!(changed.try_recv().ok(), Some(key("temp", None)));
    widgets.clear(key("temp", None));
    assert!(changed.try_recv().is_err(), "清过的再清不说");
    widgets.forget(&Instance::system("other"));
    assert!(changed.try_recv().is_err(), "别的包的不碰");
    widgets.forget(&Instance::system("wx"));
    assert_eq!(changed.try_recv().ok(), Some(key("week", None)));
    assert!(widgets.get(&key("week", None)).is_none());
}

/// 每人一份的包（施工 IM-1 中下五）：两个人的块各记各的，谁在看、程序停了清内容都只碰那一份。
#[test]
fn each_person_s_copy_has_its_own_widgets() {
    let widgets = Widgets::default();
    let person = |name: &str| AccountId::parse(name).expect("合写法");
    let theirs = |name: &str| Key {
        account: Some(person(name)),
        ..key("today", None)
    };
    let copy = |name: &str| Instance {
        package: "wx".into(),
        account: Some(person(name)),
    };
    widgets.look(&[theirs("alice"), theirs("bob")].into());
    assert!(widgets.put(theirs("alice"), shown("alice 的")));
    assert!(widgets.put(theirs("bob"), shown("bob 的")));
    assert_eq!(widgets.watched(&copy("bob")), [theirs("bob")].into());
    assert!(
        widgets.watched(&Instance::system("wx")).is_empty(),
        "一份的那一份没人看"
    );
    widgets.forget(&copy("alice"));
    assert!(
        widgets.get(&theirs("alice")).is_none(),
        "alice 那一份停了，她的清掉"
    );
    assert!(widgets.get(&theirs("bob")).is_some(), "bob 的还在");
}
