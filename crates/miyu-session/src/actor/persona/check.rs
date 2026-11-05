//! 发送前照这个会话自己判前缀变不变（施工 PX-1，`docs/construction/PX-1-前缀变动照会话判.md`）：覆盖、人格、预设那几棵树变了号，
//! 照回合开始那一套（[`super::look`]）把它的快照干算一遍、和现在的比：
//!
//! - 一样的（改的是别的人格、头像、背景、它用不到的覆盖）不用问人，看到的号对到这一次；
//! - 不一样的交回它用着的人格、预设、工具，端点照它们只列真进它前缀的改动。
//!
//! 只看文件带来的变化：工具目录当没换代（照上次拼快照时那一代）、扩展插的字当没变，那两样回合开始照旧换、照旧不问人
//! （施工 OV-1 下只为文件问）。干算在阻塞线程里，不挡收件箱，不建策略、不存 blob；只在号变了的时候端点才来问。

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::oneshot;

use miyu_policy::Snapshot;

use super::recast::Wanted;
use super::{Actor, Seen, follows_files, look};
use crate::blocking::blocking;
use crate::prefix::{PrefixCheck, Touches};

impl Actor {
    /// 照号 `version` 干算一遍，答给 `reply`。不跟着文件换快照的会话（整理的、fork 子代理）、没有人格快照的（测试里造的）不变。
    pub(in crate::actor) fn check_prefix(&self, version: u64, reply: oneshot::Sender<PrefixCheck>) {
        let Some(refresh) = self.persona.clone().filter(follows_files) else {
            super::super::answer(reply, PrefixCheck::Unchanged);
            return;
        };
        let values = self.config.current().resolved.values();
        let now = refresh
            .seen
            .clone()
            .unwrap_or_else(|| refresh.tools.edition());
        let asked: BTreeMap<String, Option<String>> = refresh
            .snapshot
            .appended
            .iter()
            .map(|(key, text)| (key.clone(), Some(text.clone())))
            .collect();
        let project = self.session.project().map(str::to_string);
        let wanted = Wanted::of(&self.session, &refresh.snapshot, refresh.child);
        let prefix = Arc::clone(&refresh.prefix);
        let touches = touches(&refresh.snapshot);
        tokio::spawn(async move {
            let seen = blocking(move || {
                look(
                    &refresh,
                    &values,
                    &now,
                    (asked, project, wanted, true, true),
                )
            })
            .await;
            let found = match seen {
                Seen::Same => {
                    prefix.see(version);
                    PrefixCheck::Unchanged
                }
                Seen::Would | Seen::Swapped(..) => PrefixCheck::Changed(touches),
                // 读不了、拼不成的回合开始也照旧，前缀不变；号不对上，下次再看。
                Seen::Unreadable(_) | Seen::Kept(_) => PrefixCheck::Unchanged,
            };
            super::super::answer(reply, found);
        });
    }
}

/// 快照 `snapshot` 的前缀用到的：人格、预设的编号，工具面上的工具名。
fn touches(snapshot: &Snapshot) -> Touches {
    Touches {
        persona: snapshot.persona.clone(),
        preset: snapshot.preset.as_ref().map(|pin| pin.id.clone()),
        tools: snapshot
            .tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect(),
    }
}
