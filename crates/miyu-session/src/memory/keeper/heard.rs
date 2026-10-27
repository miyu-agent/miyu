//! 这个会话里最近听到的（施工 E-1 修三、E-4 上）：人话的 `venue` 里她此刻叫什么（`my_name`，认她）、这个场所此刻叫什么
//! （`place`，群名，记下时存进 `Saved::place`）。会话记下一批事件时（`actor/store.rs`）记忆的端口记住最新的，载入、换人格时照
//! 读回的日志接上（`memory.rs` 的 `connect`）。

use std::collections::BTreeSet;
use std::sync::PoisonError;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;

use super::Keeper;

/// 最近听到的两样：没听到过的没有。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Heard {
    /// 她在这个场所里叫什么（施工 E-1 修三），原样的。
    pub(crate) my_name: Option<String>,
    /// 这个场所叫什么（施工 E-4 上），原样的。
    pub(crate) place: Option<String>,
}

impl Heard {
    /// `events` 里最后一条带着它的人话里的那两样，各找各的。
    pub(crate) fn last(events: &[Event]) -> Heard {
        Heard {
            my_name: super::super::her::last_called(events),
            place: events.iter().rev().find_map(place).map(str::to_string),
        }
    }
}

/// 一条人话里这个场所叫什么（施工 E-4 上）。
pub(crate) fn place(event: &Event) -> Option<&str> {
    let Body::MessageUser(message) = &event.body else {
        return None;
    };
    message.venue.as_ref()?.place.as_deref()
}

/// `events` 里每一条带着群名的人话（施工 E-4 上）：序号和群名，照先后。抽取照它给每一轮找那时的群名（[`place_at`]）。
pub(crate) fn places(events: &[Event]) -> Vec<(Seq, String)> {
    events
        .iter()
        .filter_map(|event| Some((event.seq, place(event)?.to_string())))
        .collect()
}

/// 到第 `upto` 条为止最后听到的群名（施工 E-4 上）：一轮的群名是到她答完那一条为止最近的那个；之前都没有的没有。
pub(crate) fn place_at(places: &[(Seq, String)], upto: Seq) -> Option<String> {
    places
        .iter()
        .rev()
        .find(|(seq, _)| *seq <= upto)
        .map(|(_, place)| place.clone())
}

impl Keeper {
    /// 这一间里她叫什么（施工 E-1 修，`her.rs`）：照 `fold` 过的，没交过的是空的。这个会话里她最近在场所里叫的那个也算
    /// （施工 E-1 修三）。
    pub(crate) fn her(&self) -> BTreeSet<String> {
        let mut names = self.memory.her(&self.room);
        if let Some(name) = self
            .heard()
            .my_name
            .map(|name| super::super::her::fold(&name))
            && !name.is_empty()
        {
            names.insert(name);
        }
        names
    }

    /// 这个会话里最近听到的群名（施工 E-4 上）：她当场记的时候存进去；没有的没有。
    pub(crate) fn place(&self) -> Option<String> {
        self.heard().place
    }

    /// 记下的事件 `events` 里有那两样的，各记住最后那一个；没有的不动。
    pub(crate) fn hear(&self, events: &[Event]) {
        self.called(Heard::last(events));
    }

    /// 这个会话里最近听到 `heard`（施工 E-1 修三、E-4 上）：有的换上，没有的不动。
    pub(crate) fn called(&self, heard: Heard) {
        let mut here = self.here.lock().unwrap_or_else(PoisonError::into_inner);
        if heard.my_name.is_some() {
            here.my_name = heard.my_name;
        }
        if heard.place.is_some() {
            here.place = heard.place;
        }
    }

    fn heard(&self) -> Heard {
        self.here
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
