//! 这时的上下文一块块的（施工 CX-1 上，`session.context`）：内核的明细加上照现在的快照拆的 system、标的工具面。只看不换。
//! 施工 CX-1 下起 system 照快照原来的料重拼一遍、照拼的先后全拆（[`relaid`]），对不上的才照单存的几块拆（[`split`]）。

use tokio::sync::oneshot;

use miyu_kernel::estimate;
use miyu_policy::{PersonaTexts, Snapshot};

use super::Actor;
use crate::blocking::blocking;
use crate::snapshot::{Layout, Parts, laid, pieces};
use crate::{Context, SystemPart};

impl Actor {
    /// 这时的上下文一块块的：内核的明细当场要，system 照快照原来的料重拼着拆在阻塞线程里做，不挡收件箱。没有人格快照的
    /// （测试里造的）system 只有其他、工具面是空的。
    pub(in crate::actor) fn context(&self, reply: oneshot::Sender<Context>) {
        let breakdown = self.session.breakdown();
        let Some(refresh) = self.persona.clone() else {
            let system = vec![(SystemPart::Other, breakdown.system)];
            super::super::answer(
                reply,
                Context {
                    breakdown,
                    system,
                    offered: Vec::new(),
                },
            );
            return;
        };
        tokio::spawn(async move {
            let context = blocking(move || {
                let snapshot = &refresh.snapshot;
                let system = match relaid(&refresh) {
                    Some(layout) => pieces(&layout, breakdown.system),
                    None => split(snapshot, breakdown.system),
                };
                let offered = miyu_policy::offered(&snapshot.tools, snapshot.core.loading.as_ref());
                Context {
                    breakdown,
                    system,
                    offered,
                }
            })
            .await;
            super::super::answer(reply, context);
        });
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

/// 照快照原来的料重拼一遍（人设照这时的文件），和快照的 system 逐字节一样的照它；不一样的（旧核心造的会话、人设改过，施工
/// CX-2）逐块到快照的 system 里照先后找原文（[`matched`]）。拼不出来的（人设文件没了、
/// 裸的）没有，照 [`split`] 拆。读文件，在阻塞线程里调。
pub(super) fn relaid(refresh: &super::Refresh) -> Option<Layout> {
    let (new, layout) = laid(&refresh.resources, parts_of(refresh)?).ok()?;
    let layout = layout?;
    let old = &refresh.snapshot.system;
    if new.system == *old {
        return Some(layout);
    }
    Some(matched(old, &new.system, &layout))
}

/// 照 `fresh` 的一块块（`layout`）到 `old` 里照先后找：每块去掉前面的空行找原文，找到的从上一块的末尾算起（连着它前面的
/// 空行）。块的先后是定的，夹在两块中间跳过的那一截，是没找到的那几块的旧字（旧核心的说明、改过的人设），归给其中第一块；
/// 没有块该在那儿的（以前多拼的一段），归其余。
pub(super) fn matched(old: &str, fresh: &str, layout: &Layout) -> Layout {
    let mut found: Layout = Vec::new();
    let mut at = 0;
    let mut missing: Option<SystemPart> = None;
    for (part, range) in layout {
        let text = fresh[range.clone()].trim_start_matches('\n');
        if text.is_empty() {
            continue;
        }
        let Some(offset) = old[at..].find(text) else {
            missing.get_or_insert_with(|| part.clone());
            continue;
        };
        let start = at + offset;
        let lead = old[at..start].trim_end_matches('\n').len() + at;
        if lead > at {
            let owner = missing.take().unwrap_or(SystemPart::Other);
            found.push((owner, at..lead));
        }
        missing = None;
        found.push((part.clone(), lead..start + text.len()));
        at = start + text.len();
    }
    if at < old.len() {
        found.push((missing.unwrap_or(SystemPart::Other), at..old.len()));
    }
    found
}

/// 快照原来的料：人设照这时的文件（找不到的、裸的没有），别的照快照里记的。
pub(super) fn parts_of(refresh: &super::Refresh) -> Option<Parts> {
    let old = &refresh.snapshot;
    if old.bare {
        return None;
    }
    let texts = match &old.persona {
        Some(id) => refresh.personas.find(id).ok()?.texts,
        None => PersonaTexts::default(),
    };
    Some(Parts {
        name: old.persona.clone(),
        texts,
        attended: old.attended,
        face: old.tools.clone(),
        memory: old.memory.clone(),
        child: refresh.child,
        preset: old.preset.clone(),
        roleplay: old.reminder.is_some(),
        group: old.group.as_ref().map(|chat| {
            (
                chat.offset,
                chat.recent.as_ref().and_then(|words| words.limit),
            )
        }),
        foreground: old.foreground,
        knowledge: old.knowledge.clone(),
        skills: old.skills.clone(),
        profile: old.profile.clone(),
        project: old.project.clone(),
        appended: old.appended.clone(),
        bare: false,
    })
}
