//! 她自己叫什么（施工 E-1 修，设计 42 第四节）：认识的人不记她自己。群里大家照人格的名字叫她，抽取那一段里她说的话标着
//! `You`，模型照样会写出「Miyu 是群管理」这种 `who` 是她名字的认识的人，合出一张她自己的卡；群友的群名片和她同名的，照名字
//! 认人还会把她的事记到这个人头上。所以每一间记下她叫什么（人格的名字，几种语言都算），抽取、她当场记、整理认人的时候
//! `who` 是她名字的不当认识的人：改记成 `episode`（关于她自己的事，留着搜得到），不认成谁的平台编号，不合卡。
//!
//! 名字是会话造的、载入的时候照人格交的（[`Memory::name_her`]），端点照人格找那一间时也交；没交过的那一间照没有名字算，
//! 和以前一样。只在内存里，不落盘：人格改了名字，下一次开会话就换。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use miyu_config::phrases::Label;
use miyu_recall::extract::Candidate;
use miyu_store::recall::Room;

use super::Memory;

/// 不当认识的人记的那一条改成这一类：关于她自己的事。
pub(crate) const SELF_CLASS: &str = "episode";

/// 每一间她叫什么：比的时候照 [`fold`] 过的。
#[derive(Debug, Default)]
pub(crate) struct Selves(Mutex<BTreeMap<Room, BTreeSet<String>>>);

impl Selves {
    fn set(&self, room: &Room, names: BTreeSet<String>) {
        let mut rooms = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match names.is_empty() {
            true => rooms.remove(room),
            false => rooms.insert(room.clone(), names),
        };
    }

    fn get(&self, room: &Room) -> BTreeSet<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(room)
            .cloned()
            .unwrap_or_default()
    }
}

impl Memory {
    /// 记下 `room` 那一间里她叫什么（人格的名字 `name`，几种语言的都算）：没有名字的（无人格、人格没写名字）清掉。
    pub fn name_her(&self, room: &Room, name: Option<&Label>) {
        self.selves.set(room, names(name));
    }

    /// `room` 那一间里她叫什么，照 [`fold`] 过的；没交过的是空的。
    pub(crate) fn her(&self, room: &Room) -> BTreeSet<String> {
        self.selves.get(room)
    }
}

/// 人格的名字 `name` 里的几个叫法，照 [`fold`] 过的。
fn names(name: Option<&Label>) -> BTreeSet<String> {
    let all: Vec<&str> = match name {
        Some(Label::One(text)) => vec![text.as_str()],
        Some(Label::Each(phrases)) => phrases.values().map(String::as_str).collect(),
        None => Vec::new(),
    };
    all.into_iter()
        .map(fold)
        .filter(|name| !name.is_empty())
        .collect()
}

/// 比名字的写法：去掉前后空白、不分大小写（中文照原样）。
pub(crate) fn fold(name: &str) -> String {
    name.trim().to_lowercase()
}

/// `who` 是不是她（`me` 是照 [`fold`] 过的她的名字）。
pub(crate) fn is_her(me: &BTreeSet<String>, who: &str) -> bool {
    !me.is_empty() && me.contains(&fold(who))
}

/// 抽出来、她记的一条认识的人说的是她自己的：改记成 [`SELF_CLASS`]、不写是谁。交回改了没有。
pub(crate) fn unperson(candidate: &mut Candidate, me: &BTreeSet<String>) -> bool {
    let her = candidate.class == miyu_recall::PERSON
        && candidate.who.as_deref().is_some_and(|who| is_her(me, who));
    if her {
        candidate.class = SELF_CLASS.to_string();
        candidate.who = None;
    }
    her
}

#[cfg(test)]
mod tests;
