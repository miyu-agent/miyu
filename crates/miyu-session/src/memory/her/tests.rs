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
