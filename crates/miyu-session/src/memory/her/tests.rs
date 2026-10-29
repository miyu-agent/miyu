//! 她自己叫什么（施工 E-1 修）：几种语言的名字都算，比的时候不分大小写、去掉前后空白；抽出来的认识的人说的是她的改记成
//! `episode`，群友的群名片和她同名的也不把她的事记到这个人头上。

use std::collections::{BTreeMap, BTreeSet};

use miyu_config::phrases::Label;
use miyu_kernel::event::Event;
use miyu_recall::extract::Candidate;

use super::*;
use crate::memory::extract::{Voices, place_people};

fn me() -> BTreeSet<String> {
    names(Some(&Label::One("Miyu".into())))
}

fn one(class: &str, who: Option<&str>) -> Candidate {
    Candidate {
        class: class.into(),
        text: "一件事".into(),
        turn: 5,
        about: None,
        who: who.map(str::to_string),
        people: Vec::new(),
    }
}

/// 群里的一句：`id` 以群名片 `name` 说的。
fn said(n: u64, id: &str, name: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{n},"at":"2026-10-09T03:00:00.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5","id":"{id}"}},"body":{{"blocks":[{{"type":"text","text":"话"}}],"venue":{{"msg":"{n}","name":"{name}"}}}}}}"#
    ))
    .expect("合写法")
}

#[test]
fn every_name_of_the_persona_counts_folded() {
    let each = Label::Each(BTreeMap::from([
        ("zh".to_string(), "美羽".to_string()),
        ("en".to_string(), " Miyu ".to_string()),
    ]));
    assert_eq!(
        names(Some(&each)),
        BTreeSet::from(["miyu".to_string(), "美羽".to_string()])
    );
    assert!(names(None).is_empty(), "没有名字的没有");
    assert!(is_her(&me(), " MIYU "), "不分大小写、去掉前后空白");
    assert!(!is_her(&me(), "Miyu2"));
    assert!(!is_her(&BTreeSet::new(), ""), "没有名字的谁都不是她");
}

#[test]
fn a_person_who_is_her_becomes_an_episode() {
    let mut her = one("person", Some("miyu"));
    assert!(unperson(&mut her, &me()));
    assert_eq!((her.class.as_str(), her.who), (SELF_CLASS, None));
    let mut other = one("person", Some("小林"));
    assert!(!unperson(&mut other, &me()));
    assert_eq!(other.who.as_deref(), Some("小林"));
    let mut user = one("user", Some("Miyu"));
    assert!(!unperson(&mut user, &me()), "别的类不管");
}

/// 旧主群那种片段：群里叫她 Miyu，有个群友把群名片也改成了「Miyu」。模型写出 `who` 是 Miyu 的认识的人：是她的事，不进
/// 那个群友的名下；正常的群友照旧认成平台编号；模型写的就是那个群友的平台编号的照旧是他。
#[test]
fn her_name_is_never_placed_on_a_member_who_took_it() {
    let events = [said(1, "qq:2", "小林"), said(2, "qq:3888", "Miyu")];
    let names = Voices::of(&events, Some(4096)).names;
    assert_eq!(
        names.get("Miyu"),
        Some(&Some("qq:3888".to_string())),
        "那个群友用过这个名字"
    );
    let mut found = vec![
        one("person", Some("Miyu")),
        one("person", Some("小林")),
        one("person", Some("qq:3888")),
    ];
    place_people(&mut found, &names, &me());
    let placed: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|one| (one.class.as_str(), one.who.as_deref()))
        .collect();
    assert_eq!(
        placed,
        [
            ("episode", None),
            ("person", Some("qq:2")),
            ("person", Some("qq:3888")),
        ]
    );
}

/// 群里的一句，带着她这时在群里叫什么（施工 E-1 修三）。
fn said_to(n: u64, id: &str, name: &str, my_name: &str) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{n},"at":"2026-10-09T03:00:00.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:5","id":"{id}"}},"body":{{"blocks":[{{"type":"text","text":"话"}}],"venue":{{"msg":"{n}","name":"{name}","my_name":"{my_name}"}}}}}}"#
    ))
    .expect("合写法")
}

/// 她在群里被叫过的名字：这一段里出现过的都算（照 `fold` 过的），最近的那个是最后一条带着的。
#[test]
fn names_she_is_called_in_a_venue_are_read_from_messages() {
    let events = [
        said_to(1, "qq:2", "小林", " 米羽 "),
        said(2, "qq:3", "阿杰"),
        said_to(3, "qq:2", "小林", "MiyuBot"),
    ];
    assert_eq!(
        called_in(&events),
        BTreeSet::from(["米羽".to_string(), "miyubot".to_string()])
    );
    assert_eq!(last_called(&events).as_deref(), Some("MiyuBot"));
    assert_eq!(last_called(&[said(1, "qq:2", "小林")]), None, "没带的没有");
}

/// 人格叫 Miyu、群里叫她「米羽」：模型写出 `who` 是米羽的认识的人是她的事，记成 episode；有个群友群名片也叫「米羽」的，
/// 不拿来认他。
#[test]
fn a_venue_name_counts_as_her_even_when_a_member_took_it() {
    let events = [
        said_to(1, "qq:2", "小林", "米羽"),
        said_to(2, "qq:3888", "米羽", "米羽"),
    ];
    let voices = Voices::of(&events, Some(4096));
    let mut me = me();
    me.extend(voices.me.clone());
    let mut found = vec![one("person", Some("米羽")), one("person", Some("小林"))];
    place_people(&mut found, &voices.names, &me);
    let placed: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|one| (one.class.as_str(), one.who.as_deref()))
        .collect();
    assert_eq!(placed, [("episode", None), ("person", Some("qq:2"))]);
}

/// 她当场记的时候认她（施工 E-1 修三）：会话记下的人话带着她在场所里叫什么的，这个会话的 `Keeper`（连同复制出来的）记住最近
/// 的那个，和人格的名字一起算；没带的不动，别的会话的不受影响。
#[test]
fn a_keeper_remembers_what_she_was_last_called_here() {
    use miyu_store::env::{Env, Platform};
    use miyu_store::root::DataRoot;

    use crate::memory::{Keeper, Listener};

    let scratch = std::env::temp_dir().join(format!("miyu-her-{}", std::process::id()));
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    let memory = Memory::new(&root, None);
    let room = Room::persona("engineer");
    memory.name_her(&room, Some(&Label::One("Miyu".into())));
    let listener = || {
        Listener::Owner(miyu_kernel::origin::By::Person(
            miyu_kernel::origin::Person::default(),
        ))
    };
    let here = Keeper::new(&memory, room.clone(), listener());
    let copy = here.clone();
    let elsewhere = Keeper::new(&memory, room, listener());

    here.hear(&[said_to(1, "qq:2", "小林", "米羽"), said(2, "qq:3", "阿杰")]);
    here.hear(&[said(3, "qq:3", "阿杰")]);
    let both = BTreeSet::from(["miyu".to_string(), "米羽".to_string()]);
    assert_eq!(here.her(), both, "没带的不动");
    assert_eq!(copy.her(), both, "复制出来的共用");
    assert_eq!(here.as_owner().her(), both, "主人那边的一份也是");
    assert_eq!(elsewhere.her(), me(), "别的会话的不受影响");
    here.hear(&[said_to(4, "qq:2", "小林", "Mi")]);
    assert_eq!(
        here.her(),
        BTreeSet::from(["miyu".to_string(), "mi".to_string()]),
        "照最近的"
    );
    drop(std::fs::remove_dir_all(&scratch));
}
