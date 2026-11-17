//! 这一轮的话从哪来（施工 VN-1，项目主人 2026-11-17 本机验收第 16 条）：本机会话里，人在终端界面、网页、命令行、终端集成上说
//! 的话，到点的定时任务、循环、人生叫醒的（没有人在场），她分不清，在网页上要她念一句，她合成了文件发到 QQ 群里。回合开始照
//! 开这一轮的那句话的 `by`（[`crate::origin::Person::head`]、[`By::Schedule`]）挑一句，和有效历史里同一类的最近一块逐字节相同的
//! 不注入：换了地方说话才追加一块新的，前面的缓存不动（`08-上下文投影.md` C10，同环境那一块）。
//!
//! - 哪几种头有自己的一句是数据（`core/facts/channel/head/<种类>.txt`），不认识的种类、没报种类的用 `other.txt`。
//! - 别的会话、子代理、别的 harness、模块说的，回报开的一轮，不注入：上一块照旧算数，那些话自己带着标签。
//! - 场所会话（群、私聊）没有这一份：桥在 system 尾部写了场所（施工 X-20 的 `<venue>`），快照里不带这几句（`miyu-session` 拼快照时
//!   照场所去掉）。以前造的快照里也没有，那些会话不注入，前缀一字不变。

use std::collections::BTreeMap;

use super::{FactTemplates, kind, unseen};
use crate::event::ContextInjected;
use crate::history::History;
use crate::origin::By;

/// 这一类的名字。
pub const CHANNEL: &str = "channel";

/// 这一块的几句原文，没有字段：造快照时读好交进来，冻结在会话上（内核 K3）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Channels {
    /// 头的种类 → 那一句（`core/facts/channel/head/<种类>.txt`）。
    pub heads: BTreeMap<String, String>,
    /// 不在 [`Channels::heads`] 里的头、没报种类的（`core/facts/channel/other.txt`）。
    pub other: String,
    /// 到点的定时任务、循环、人生叫醒的，没有人在场（`core/facts/channel/nobody.txt`）。
    pub nobody: String,
}

impl Channels {
    /// 开这一轮的那句话的 `by` 该挑哪一句：本机的头上的人照头的种类，到点叫醒的是没人在场；别的不挑。
    fn said_by(&self, by: &By) -> Option<&str> {
        match by {
            By::Person(person) if person.via.is_none() => Some(
                person
                    .head
                    .as_ref()
                    .and_then(|head| self.heads.get(head.as_str()))
                    .unwrap_or(&self.other),
            ),
            By::Schedule(_) => Some(&self.nobody),
            _ => None,
        }
        .map(String::as_str)
    }
}

impl FactTemplates {
    /// 带上这几句（施工 VN-1）：`None` 的不注入这一块。
    #[must_use]
    pub fn with_channels(mut self, channels: Option<Channels>) -> FactTemplates {
        self.channels = channels;
        self
    }

    /// 回合开始时该不该注入这一块：`asked` 是开这一轮的那句话的 `by`（没有的不注入）。挑出的那一句和有效历史里内核记的最近
    /// 一块一样的不注入。没有这几句的（场所会话、以前造的快照）、安静的不注入。
    pub fn channel(&self, history: &History, asked: Option<&By>) -> Option<ContextInjected> {
        let channels = self.channels.as_ref().filter(|_| !self.quiet)?;
        let text = channels.said_by(asked?)?;
        let fact = ContextInjected {
            refs: Vec::new(),
            after: false,
            kind: kind(CHANNEL),
            text: text.to_string(),
        };
        unseen(history, &By::Kernel, &fact).then_some(fact)
    }
}
