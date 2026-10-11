//! 这一段里人这边每一句是谁说的（施工 R-13 上，`docs/blueprint/memory.md` 第六条第 4 款）：记忆只有一份，记的是她遇到的每个人
//! 和事，每一条写清是谁、谁说的（2026-10-11 项目主人定）。所以拼给抽取的这一段要标出说话的人：私聊里主人写 `Owner`、别人写
//! 名字；群里一行本来就带着名字和记号（`miyu-assemble` 的群消息那一行），原样放。不是人说的（别的会话、harness 发来的）不进：
//! 事实只从人说的话里取（第五条）。

use std::collections::BTreeMap;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;

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
}

impl Voices {
    /// 照这一段的事件 `events` 认：人（主人）说的写 `Owner`，平台上的人写他在场所里的名字、没有的写平台编号；`group` 是群
    /// 会话的群聊近况预算，群会话里人说的都原样放。
    pub(crate) fn of(events: &[Event], group: Option<usize>) -> Voices {
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
        }
    }
}
