//! 这一段里人这边每一句是谁说的（施工 R-13 上，`docs/blueprint/memory.md` 第六条第 4 款）：记忆只有一份，记的是她遇到的每个人
//! 和事，每一条写清是谁、谁说的（2026-10-11 项目主人定）。所以拼给抽取的这一段要标出说话的人：私聊里主人写 `Owner`、别人写
//! 名字；群里一行本来就带着名字和记号（`miyu-assemble` 的群消息那一行），原样放。不是人说的（别的会话、harness 发来的）不进：
//! 事实只从人说的话里取（第五条）。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;

use super::prefix::Prefix;

/// 人这边的一句是谁说的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Voice {
    /// 主人。
    Owner,
    /// 对面那个人：名字，没有的是平台编号。
    Named(String),
    /// 群里的一行：本来就带着名字和记号，原样放。
    Line,
}

/// 这一段里人这边每一句是谁说的，群里每一轮带多少。
#[derive(Debug, Default)]
pub(crate) struct Voices {
    /// 照序号；不在里面的不是人说的，不进这一段。
    pub(crate) said: BTreeMap<Seq, Voice>,
    /// 群会话：每一轮只带离它最近、装得进这么多字节的那几句（群聊近况的预算：她当时看到的大致就是这些），她没开口的闲聊不带
    /// （2026-10-11 项目主人定「只抽她参与的几轮」）。不是群会话的没有。
    pub(crate) budget: Option<usize>,
    /// 群会话的群聊近况最多几条（施工 O-13 再补）：每一轮也最多带这么多句。没写的、不是群会话的没有。
    pub(crate) limit: Option<usize>,
    /// 这一段里说话的人怎么叫（施工 E-1 上）：名字、平台编号对着他的平台编号；主人（`Owner`、对应表认出的本人在群里的名字）
    /// 对着没有。抽出来的认识的人照它把名字换成平台编号。
    pub(crate) names: BTreeMap<String, Option<String>>,
    /// 这一段里她在场所里被叫过的名字（施工 E-1 修三，人话的 `venue.my_name`），照 `fold` 过的：和人格的名字一起算她。
    pub(crate) me: BTreeSet<String>,
    /// 这一段里带着群名的人话（施工 E-4 上，人话的 `venue.place`）：序号和群名，照先后。每一轮记下的照它找那时的群名。
    pub(crate) places: Vec<(Seq, String)>,
}

impl Voices {
    /// 照这一段的事件 `events` 认：人（主人）说的写 `Owner`，平台上的人写他在场所里的名字、没有的写平台编号；`group` 是群
    /// 会话的群聊近况预算，群会话里人说的都原样放。
    pub(crate) fn of(events: &[Event], group: Option<usize>) -> Voices {
        Voices::continued(None, events, group)
    }

    /// 同 [`Voices::of`]，只是 `events` 接在 `before`（读起的那一处以前的日志里认出来的，施工 E-5 补）后面：名字、她被叫过的
    /// 名字、群名照它接着认，和整份一起认的一样。
    pub(crate) fn continued(
        before: Option<&Prefix>,
        events: &[Event],
        group: Option<usize>,
    ) -> Voices {
        let mut names = before.map_or_else(owner_only, |before| before.names.as_ref().clone());
        learn(events, |name, id| {
            names.insert(name, id);
        });
        let mut me = before
            .map(|before| before.me.as_ref().clone())
            .unwrap_or_default();
        me.extend(crate::memory::her::called_in(events));
        let mut places: Vec<(Seq, String)> = before
            .and_then(|before| before.place.clone())
            .into_iter()
            .collect();
        places.extend(crate::memory::keeper::places(events));
        let said = events
            .iter()
            .filter_map(|event| {
                let Body::MessageUser(message) = &event.body else {
                    return None;
                };
                let voice = match (&event.by, group) {
                    (By::Person(_) | By::External(_), Some(_)) => Voice::Line,
                    (By::Person(_), None) => Voice::Owner,
                    (By::External(external), None) => Voice::Named(
                        message
                            .venue
                            .as_ref()
                            .and_then(|venue| venue.name.clone())
                            .unwrap_or_else(|| external.id.as_str().to_string()),
                    ),
                    _ => return None,
                };
                Some((event.seq, voice))
            })
            .collect();
        Voices {
            said,
            budget: group,
            limit: None,
            names,
            me,
            places,
        }
    }
}

/// 只有主人那一行的名字表：一句都没认过的起点。
pub(crate) fn owner_only() -> BTreeMap<String, Option<String>> {
    BTreeMap::from([(OWNER.to_string(), None)])
}

/// 照 `events` 里的人话认名字，一条条交给 `put`（名字、对着的平台编号），照先后：平台上的人的名字、平台编号对着他的平台编号，
/// 主人在场所里的名字对着没有。后交的盖掉先交的。
pub(crate) fn learn(events: &[Event], mut put: impl FnMut(String, Option<String>)) {
    for event in events {
        let Body::MessageUser(message) = &event.body else {
            continue;
        };
        let name = message.venue.as_ref().and_then(|venue| venue.name.clone());
        match &event.by {
            By::External(external) => {
                let id = external.id.as_str().to_string();
                if let Some(name) = name {
                    put(name, Some(id.clone()));
                }
                put(id.clone(), Some(id));
            }
            By::Person(_) => {
                if let Some(name) = name {
                    put(name, None);
                }
            }
            _ => {}
        }
    }
}

/// 拼给抽取的那一段里主人那一行的记号（`extract/owner.txt`）。
const OWNER: &str = "Owner";
