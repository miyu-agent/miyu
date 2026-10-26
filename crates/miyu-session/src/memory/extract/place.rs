//! 抽出来的几条说的是谁、涉及谁（施工 E-1 上，E-1 修、E-1 修四）：照这一段的说话人把模型写的名字换成平台编号。

use std::collections::{BTreeMap, BTreeSet};

use miyu_recall::extract::Candidate;
use miyu_recall::{EPISODE, PERSON};

/// 抽出来的认识的人说的是谁（施工 E-1 上，设计 42 PN2）：模型照这一段写的名字，说话的人里有他的换成平台编号（`names`，
/// `Plan::Ask` 的那一格）；说的是主人的改记成 `user`、不写是谁；对不上的照名字留着。别的类不带是谁。
///
/// 说的是她自己的（`who` 是她的名字 `me`，施工 E-1 修）先挑出来改记成 `episode`、不写是谁：不照名字认，群友的群名片和她同名
/// 的也不会把她的事记到这个人头上。
///
/// 施工 E-1 修四：
/// - 名字对不上谁的，是这一段里恰好一个人的名字的开头（群名片后面带着一串说明，模型只写了前半截）就认成他（[`place`]）。
/// - 发生过的事（`episode`）涉及谁照同一个认法换成平台编号：她自己、主人不算，对不上的照名字留；别的类没有。
/// - 关于主人（`user`）的一条说的是她自己的（照名字起头，`her::about_her`）改记成 `episode`。
pub(crate) fn place_people(
    found: &mut [Candidate],
    names: &BTreeMap<String, Option<String>>,
    me: &BTreeSet<String>,
) {
    for candidate in found {
        if candidate.class == USER && crate::memory::her::about_her(&candidate.text, me) {
            candidate.class = crate::memory::her::SELF_CLASS.to_string();
        }
        candidate.people = match candidate.class == EPISODE {
            true => involved(&candidate.people, names, me),
            false => Vec::new(),
        };
        if candidate.class != PERSON {
            candidate.who = None;
            continue;
        }
        if crate::memory::her::unperson(candidate, me) {
            continue;
        }
        match candidate
            .who
            .as_deref()
            .and_then(|who| place(who, names, me))
        {
            Some(Some(id)) => candidate.who = Some(id.clone()),
            Some(None) => {
                candidate.class = USER.to_string();
                candidate.who = None;
            }
            None => {}
        }
    }
}

/// 照前半截认人，名字至少要有这么多个字。
const PREFIX: usize = 2;

/// 名字 `who` 认成谁（施工 E-1 修四）：这一段里有人这样叫的照他（主人是 `None`）；没有的，是这一段里恰好一个人的名字的开头
/// （去掉前后空白、不分大小写，至少 [`PREFIX`] 个字）的照他：群名片带着一长串说明，模型只写了前半截。对上几个不同的人、对上
/// 的是她的名字、平台编号本身（编号不照开头认）的不算，写的是她名字的开头的也不照开头认。都对不上的是 `None`，照名字留。
fn place<'a>(
    who: &str,
    names: &'a BTreeMap<String, Option<String>>,
    me: &BTreeSet<String>,
) -> Option<&'a Option<String>> {
    if let Some(found) = names.get(who) {
        return Some(found);
    }
    let head = crate::memory::her::fold(who);
    if head.chars().count() < PREFIX || me.iter().any(|name| name.starts_with(&head)) {
        return None;
    }
    let mut hit: Option<&Option<String>> = None;
    for (name, target) in names {
        let folded = crate::memory::her::fold(name);
        let id = target.as_deref() == Some(name.as_str());
        if id
            || folded == head
            || !folded.starts_with(&head)
            || crate::memory::her::is_her(me, name)
        {
            continue;
        }
        match hit {
            Some(seen) if seen != target => return None,
            _ => hit = Some(target),
        }
    }
    hit
}

/// 一条发生过的事涉及的几个人 `people` 认成谁（施工 E-1 修四）：同 [`place`]；她自己、主人不算，对不上的照名字留，去重。
fn involved(
    people: &[String],
    names: &BTreeMap<String, Option<String>>,
    me: &BTreeSet<String>,
) -> Vec<String> {
    let mut placed: Vec<String> = Vec::new();
    for name in people {
        if crate::memory::her::is_her(me, name) {
            continue;
        }
        let one = match place(name, names, me) {
            Some(Some(id)) => id.clone(),
            Some(None) => continue,
            None => name.clone(),
        };
        if !placed.contains(&one) {
            placed.push(one);
        }
    }
    placed
}

/// 关于主人的那一类。
const USER: &str = "user";

#[cfg(test)]
mod tests;
