//! 这时的上下文一块块的（施工 CX-1 上，`session.context`）：内核的明细加上照现在的快照拆的 system、标的工具面。只看不换。

use miyu_kernel::estimate;
use miyu_policy::Snapshot;

use super::Actor;
use crate::{Context, SystemPart};

impl Actor {
    /// 这时的上下文一块块的。没有人格快照的（测试里造的）system 只有其他、工具面是空的。
    pub(in crate::actor) fn context(&self) -> Context {
        let breakdown = self.session.breakdown();
        let (system, offered) = match &self.persona {
            Some(refresh) => (
                split(&refresh.snapshot, breakdown.system),
                miyu_policy::offered(
                    &refresh.snapshot.tools,
                    refresh.snapshot.core.loading.as_ref(),
                ),
            ),
            None => (vec![(SystemPart::Other, breakdown.system)], Vec::new()),
        };
        Context {
            breakdown,
            system,
            offered,
        }
    }
}

/// 把 system 拆开：快照里单存的几块，原文在拼好的 system 里找得到的才算（找不到的是快照和发的对不上，不认），剩下的都是其他。
pub(super) fn split(snapshot: &Snapshot, total: u64) -> Vec<(SystemPart, u64)> {
    let stored = [
        (SystemPart::Skills, snapshot.skills.as_deref()),
        (SystemPart::Knowledge, snapshot.knowledge.as_deref()),
        (SystemPart::Profile, snapshot.profile.as_deref()),
        (SystemPart::Project, snapshot.project.as_deref()),
    ]
    .into_iter()
    .filter_map(|(part, text)| text.map(|text| (part, text)))
    .chain(
        snapshot
            .appended
            .iter()
            .map(|(id, text)| (SystemPart::Appended(id.clone()), text.as_str())),
    );
    let mut parts = Vec::new();
    let mut known = 0u64;
    for (part, text) in stored {
        if text.is_empty() || !snapshot.system.contains(text) {
            continue;
        }
        let tokens = estimate::text(text);
        known = known.saturating_add(tokens);
        parts.push((part, tokens));
    }
    parts.push((SystemPart::Other, total.saturating_sub(known)));
    parts
}
