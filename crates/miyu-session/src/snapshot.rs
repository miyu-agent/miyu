//! 拼策略快照（施工 3-6 上起在 `open.rs`，施工 P-1 再补挪出来：换快照也照它拼）：人格的字照几层叠好的，接上随核心附带的
//! 字、工具面、记忆的范围；子会话接场所说明，群会话接格式说明（施工 O-13 中）；再接「关于你」（施工 P-9）；最后是核心的几行和风格锁
//! （`docs/designs/26-提示词.md` 第四节）。核心的几行里本机是什么系统照 [`miyu_store::env::os_name`] 写（施工 2-7 再补）。

use std::ops::Range;

use miyu_policy::{PersonaTexts, PresetPin, Snapshot, ToolEntry, compose};
use miyu_store::resources::{ResourceRoot, SourceError};

use crate::SystemPart;

/// system 一块块在哪（施工 CX-1 下）：照拼的先后，每块是它在 system 里的字节范围，连着它前面那个空行；连起来正好是整段。
pub(crate) type Layout = Vec<(SystemPart, Range<usize>)>;

/// 拼一份快照的料：人格的编号和叠好的字、有没有人能确认、工具面、记忆的范围（快照里那一格的原样，没有的是没有），是不是
/// 子会话，预设（施工 P-2 中，快照里那一格的原样；工具面、记忆的范围已经照它筛过），人设遵循提醒装了没有（施工 F-3 上：没装的不带提醒短语、
/// 风格锁），群会话钉下的时区（施工 O-13 中：比 UTC 早多少分钟；不是群会话的没有）。
pub(crate) struct Parts {
    pub(crate) name: Option<String>,
    pub(crate) texts: PersonaTexts,
    pub(crate) attended: bool,
    pub(crate) face: Vec<ToolEntry>,
    pub(crate) memory: Option<String>,
    pub(crate) child: bool,
    pub(crate) preset: Option<PresetPin>,
    pub(crate) roleplay: bool,
    /// 群会话：造会话时的时区（比 UTC 早多少分钟）、群聊近况最多带几条（施工 O-13 再补，没有的只照字节预算）。
    pub(crate) group: Option<(i32, Option<u64>)>,
    /// 后台运行关着（施工 T-1 上，[`crate::agents::Agents::foreground`]）。
    pub(crate) foreground: bool,
    /// 她查得到的知识库那一块（施工 K-1 补，[`crate::knowledge::listing`]）：工具面上有知识库那两件的才有。
    pub(crate) knowledge: Option<String>,
    /// 她用得上的技能那一块（施工 X-10 上，[`crate::skills::listing`]）：工具面上有 `read` 的才有；换人格时照旧快照的。
    pub(crate) skills: Option<String>,
    /// 「关于你」那一块（施工 P-9，`miyu_policy::Profile::block`）：开会话时照这个人写的拼；换人格时照旧快照的。
    pub(crate) profile: Option<String>,
    /// 项目的附加提示词那一块（施工 H-3，`miyu_policy::project_block`）：开会话时没有，回合开始照会话现在在哪个项目拼。
    pub(crate) project: Option<String>,
    /// 扩展放在 system 尾部的那几段（施工 X-20，[`crate::appended::resolve`]）：包的编号到原文。
    pub(crate) appended: std::collections::BTreeMap<String, String>,
    /// 裸的（施工 D-1 上，整理会话，`miyu_policy::Snapshot::bare`）：拼好以后去掉 system、工具和几样开关。
    pub(crate) bare: bool,
}

/// 照 `parts` 拼一份快照，随核心附带的字从 `resources` 读这一刻的。
///
/// # Errors
///
/// 资源目录里哪一份读不了。
pub(crate) fn build(resources: &ResourceRoot, parts: Parts) -> Result<Snapshot, SourceError> {
    laid(resources, parts).map(|(snapshot, _)| snapshot)
}

/// 同 [`build`]，连同 system 一块块在哪（施工 CX-1 下）：拼出来的字一个不变，只在每一步前后量一下多了哪一截。裸的没有。
///
/// # Errors
///
/// 同 [`build`]。
pub(crate) fn laid(
    resources: &ResourceRoot,
    parts: Parts,
) -> Result<(Snapshot, Option<Layout>), SourceError> {
    let sources = resources.sources_with(parts.texts)?;
    let tells = crate::agents::clicall::tells(&parts.face);
    let mut ruler = Ruler::default();
    let mut snapshot =
        compose(parts.name.as_deref(), sources, parts.attended).with_tools(parts.face);
    ruler.mark(SystemPart::Persona, "", &snapshot.system);
    snapshot.memory = parts.memory;
    snapshot.foreground = parts.foreground;
    if !parts.roleplay {
        snapshot.reminder = None;
    }
    if parts.child {
        let venue = resources.subagent_venue()?;
        snapshot = ruler.step(SystemPart::Venue, snapshot, |it| it.with_venue(&venue));
    }
    if let Some((offset, recent)) = parts.group {
        let mut chat = resources.group_chat(offset)?;
        if let Some(words) = chat.recent.as_mut() {
            words.limit = recent;
        }
        let note = resources.group_note()?;
        snapshot = ruler.step(SystemPart::Group, snapshot, |it| it.with_group(&note, chat));
    }
    snapshot = ruler.step(SystemPart::Profile, snapshot, |it| {
        it.with_profile(parts.profile)
    });
    snapshot = ruler.step(SystemPart::Project, snapshot, |it| {
        it.with_project(parts.project)
    });
    let lines = resources.core_lines()?;
    let os = miyu_store::env::os_name();
    let shown = miyu_policy::shown::Shown::of(parts.preset.as_ref());
    snapshot = ruler.step(SystemPart::Core, snapshot, |it| {
        it.with_core_lines(&lines, os, shown)
    });
    snapshot = ruler.step(SystemPart::Clicall, snapshot, |it| {
        it.with_clicall(&lines, tells)
    });
    snapshot = ruler.step(SystemPart::Skills, snapshot, |it| {
        it.with_skills(parts.skills)
    });
    snapshot = ruler.step(SystemPart::Knowledge, snapshot, |it| {
        it.with_knowledge(parts.knowledge)
    });
    snapshot = snapshot.with_preset(parts.preset);
    snapshot = ruler.step(SystemPart::StyleLock, snapshot, |it| {
        it.with_style_lock(&lines.style_lock)
    });
    let before = snapshot.system.len();
    let appended: Vec<(String, String)> = parts
        .appended
        .iter()
        .map(|(id, text)| (id.clone(), text.clone()))
        .collect();
    snapshot = snapshot.with_appended(parts.appended);
    ruler.appended(before, &appended, snapshot.system.len());
    Ok(if parts.bare {
        (snapshot.bare(), None)
    } else {
        let layout = ruler.layout();
        (snapshot, layout)
    })
}

/// 量拼的每一步多了哪一截（施工 CX-1 下）。拼 system 只往后接：哪一步不是往后接的（以后改了拼法），整份不交。
#[derive(Default)]
struct Ruler {
    layout: Layout,
    broken: bool,
}

impl Ruler {
    /// 做一步 `step`，多出来的那一截记成 `part`；没多的不记。
    fn step(
        &mut self,
        part: SystemPart,
        snapshot: Snapshot,
        step: impl FnOnce(Snapshot) -> Snapshot,
    ) -> Snapshot {
        let before = snapshot.system.clone();
        let after = step(snapshot);
        self.mark(part, &before, &after.system);
        after
    }

    fn mark(&mut self, part: SystemPart, before: &str, after: &str) {
        if after == before {
            return;
        }
        if !after.starts_with(before) {
            self.broken = true;
            return;
        }
        self.layout.push((part, before.len()..after.len()));
    }

    /// 扩展追加的几段一步接上（`with_appended`）：照包的先后、拼 system 的规矩（去掉末尾空白、空的不接、前面空一行）
    /// 一段段量。对不上从 `before` 到 `after` 的，整份不交。
    fn appended(&mut self, before: usize, appended: &[(String, String)], after: usize) {
        let mut at = before;
        for (id, text) in appended {
            let text = text.trim_end();
            if text.is_empty() {
                continue;
            }
            let end = at + if at == 0 { 0 } else { 2 } + text.len();
            self.layout
                .push((SystemPart::Appended(id.clone()), at..end));
            at = end;
        }
        if at != after {
            self.broken = true;
        }
    }

    fn layout(self) -> Option<Layout> {
        (!self.broken).then_some(self.layout)
    }
}

/// 限额里模型那一块补上快照的预设（施工 E-M、E-M 修）：路由不知道预设，交给内核之前补上，不用预设的 `none`。造会话、载入
/// 时第一次交的那一份也要补：子代理一造好就开第一轮，没补的话第一块模型事实写成空的预设，等 actor 补上以后又注入一块。
pub(crate) fn stamp_preset(limits: &mut miyu_kernel::session::Limits, pin: Option<&PresetPin>) {
    if let Some(identity) = limits.identity.as_mut() {
        identity.preset = pin.map_or_else(|| "none".to_string(), |pin| pin.id.clone());
    }
}

/// 照一块块在哪拆（施工 CX-1 下）：每块照它那一截的字节估，逐块取整的零头（和整段估的差）并进最大的那块，加起来正好是
/// `total`；是 0 的不列。没有「其余」：全拆得开。
pub(crate) fn pieces(layout: &Layout, total: u64) -> Vec<(SystemPart, u64)> {
    // 同一类的几截（逐块认剩下的其余，施工 CX-2）并成一块，照第一次出现的先后。
    let mut parts: Vec<(SystemPart, u64)> = Vec::new();
    for (part, range) in layout {
        let tokens = (range.len() / 4) as u64;
        match parts.iter_mut().find(|(known, _)| known == part) {
            Some((_, sum)) => *sum = sum.saturating_add(tokens),
            None => parts.push((part.clone(), tokens)),
        }
    }
    let known: u64 = parts.iter().map(|(_, tokens)| tokens).sum();
    if let Some(largest) = parts.iter_mut().max_by_key(|(_, tokens)| *tokens) {
        largest.1 = largest.1.saturating_add(total.saturating_sub(known));
    }
    parts.retain(|(_, tokens)| *tokens > 0);
    parts
}
