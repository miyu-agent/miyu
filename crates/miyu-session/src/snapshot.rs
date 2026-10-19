//! 拼策略快照（施工 3-6 上起在 `open.rs`，施工 P-1 再补挪出来：换快照也照它拼）：人格的字照几层叠好的，接上随核心附带的
//! 字、工具面、记忆的范围；子会话接场所说明，群会话接格式说明（施工 O-13 中）；再接「关于你」（施工 P-9）；最后是核心的几行和风格锁
//! （`docs/designs/26-提示词.md` 第四节）。核心的几行里本机是什么系统照 [`miyu_store::env::os_name`] 写（施工 2-7 再补）。

use std::collections::BTreeSet;

use miyu_policy::features::Features;
use miyu_policy::{PersonaTexts, PresetPin, Snapshot, ToolEntry, compose};
use miyu_store::resources::{ResourceRoot, SourceError};
use miyu_tool::Catalog;

/// 拼一份快照的料：人格的编号和叠好的字、有没有人能确认、工具面、记忆的范围（快照里那一格的原样，没有的是没有），是不是
/// 子会话，预设（施工 P-2 中，快照里那一格的原样；工具面、记忆的范围已经照它筛过），有工具的功能（施工 O-18：装了没开的那一行
/// 只列它们，照完整的工具目录算，[`tooled`]；施工 F-3 上起是功能），人设遵循提醒装了没有（施工 F-3 上：没装的不带提醒短语、
/// 风格锁），群会话钉下的时区（施工 O-13 中：比 UTC 早多少分钟；不是群会话的没有）。
pub(crate) struct Parts {
    pub(crate) name: Option<String>,
    pub(crate) texts: PersonaTexts,
    pub(crate) attended: bool,
    pub(crate) face: Vec<ToolEntry>,
    pub(crate) memory: Option<String>,
    pub(crate) child: bool,
    pub(crate) preset: Option<PresetPin>,
    pub(crate) tooled: BTreeSet<String>,
    pub(crate) roleplay: bool,
    pub(crate) group: Option<i32>,
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
}

/// 工具目录 `catalog` 里有工具的功能（施工 O-18；施工 F-3 上起照装了的功能 `features` 认工具归哪个功能）：不照这次的预设筛，
/// 某个功能在目录里有工具就算。认不出功能的（没交功能的测试、没写进清单的工具）照它的包的编号。
pub(crate) fn tooled(catalog: &Catalog, features: Option<&Features>) -> BTreeSet<String> {
    catalog
        .specs()
        .filter_map(|spec| {
            let package = catalog.package_of(&spec.name)?;
            let feature = features.and_then(|features| features.of_tool(package, &spec.name));
            Some(feature.unwrap_or(package).to_string())
        })
        .collect()
}

/// 照 `parts` 拼一份快照，随核心附带的字从 `resources` 读这一刻的。
///
/// # Errors
///
/// 资源目录里哪一份读不了。
pub(crate) fn build(resources: &ResourceRoot, parts: Parts) -> Result<Snapshot, SourceError> {
    let sources = resources.sources_with(parts.texts)?;
    let mut snapshot =
        compose(parts.name.as_deref(), sources, parts.attended).with_tools(parts.face);
    snapshot.memory = parts.memory;
    snapshot.foreground = parts.foreground;
    if !parts.roleplay {
        snapshot.reminder = None;
    }
    if parts.child {
        snapshot = snapshot.with_venue(&resources.subagent_venue()?);
    }
    if let Some(offset) = parts.group {
        snapshot = snapshot.with_group(&resources.group_note()?, resources.group_chat(offset)?);
    }
    snapshot = snapshot
        .with_profile(parts.profile)
        .with_project(parts.project);
    let lines = resources.core_lines()?;
    Ok(snapshot
        .with_core_lines(&lines, miyu_store::env::os_name())
        .with_skills(parts.skills)
        .with_knowledge(parts.knowledge)
        .with_preset(parts.preset, &lines.preset_off, &parts.tooled)
        .with_style_lock(&lines.style_lock)
        .with_appended(parts.appended))
}
