//! 回合开始重新筛工具面（施工 P-2 中下、O-2 中、K-1 下，从 `persona.rs` 挪出来）：照预设、现在的目录筛给这个会话的工具，
//! 知识库的几件照这一刻查不查得到换，目录换了代照旧快照的先后一件一件对。

use miyu_config::Values;
use miyu_policy::ToolEntry;
use miyu_policy::preset::PresetFile;
use miyu_tool::{Catalog, KB_DELETE, KB_EDIT, KB_READ, KB_SAVE, KB_SEARCH};

use super::{Refresh, features};
use crate::agents::{Agents, Holds, Offers, Reach, Site, persona_choices};

/// 照预设 `file`、现在的目录 `catalog` 筛出给这个会话的工具（施工 P-2 中、O-2 中）：场所、子会话、能不能确认、记忆的范围照旧
/// 快照；新打开的 `subagent` 能选的池照这一轮的配置 `values`。
pub(super) fn offered(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: Option<&PresetFile>,
    reach: Reach,
) -> Vec<ToolEntry> {
    let old = &refresh.snapshot;
    let features = features(refresh);
    Agents::face(
        catalog,
        Site {
            venue: &refresh.venue,
            group: old.group.is_some(),
        },
        refresh.lineage.as_ref(),
        &Offers::of(values, persona_choices(&refresh.personas)),
        old.attended,
        Holds {
            memory: old.memory_scope(),
            knowledge: reach,
        },
        file.map(|file| (file, features.as_deref())),
    )
}

/// 知识库的几件（施工 K-1 下两件，K-3 加了写的三件）。
const KB_TOOLS: [&str; 5] = [KB_SEARCH, KB_READ, KB_SAVE, KB_EDIT, KB_DELETE];

/// 工具面里知识库的几件，照名字排：比快照里的和这一刻该有的一样不一样。
pub(super) fn shelf_tools(face: &[ToolEntry]) -> Vec<&str> {
    let mut names: Vec<&str> = face
        .iter()
        .map(|entry| entry.name.as_str())
        .filter(|name| KB_TOOLS.contains(name))
        .collect();
    names.sort_unstable();
    names
}

/// 知识库的几件换成这一刻该有的 `fresh` 里的（施工 K-1 下）：查得到的照现在的目录、预设补上，查不到的拿掉；别的照旧。
pub(super) fn reshelved(face: Vec<ToolEntry>, fresh: &[ToolEntry]) -> Vec<ToolEntry> {
    let shelf = |entry: &ToolEntry| KB_TOOLS.contains(&entry.name.as_str());
    face.into_iter()
        .filter(|entry| !shelf(entry))
        .chain(fresh.iter().filter(|entry| shelf(entry)).cloned())
        .collect()
}

/// 预设改了，照新的预设 `file` 重新筛工具面（施工 P-2 下）：以前就有的那几件照旧快照里的原样（描述、`subagent` 能选的池都
/// 不跟着变，改过名的照旧叫以前的名字），新打开的照现在的目录拿；提供者的工具照现在的登记（施工 O-2 中）。脚本里能不能调
/// 照新的预设（施工 M-6）：这一格不发给模型，工具说明一字不变，关了的内核就不开通道。
pub(super) fn refaced(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: &PresetFile,
    reach: Reach,
) -> Vec<ToolEntry> {
    let old = &refresh.snapshot;
    offered(refresh, values, catalog, Some(file), reach)
        .into_iter()
        .map(|entry| {
            if catalog.provided(&entry.name) {
                return entry;
            }
            old.tools
                .iter()
                .find(|kept| {
                    catalog
                        .get(&kept.name)
                        .is_some_and(|tool| tool.spec().name == entry.name)
                })
                .map(|kept| ToolEntry {
                    clicall: entry.clicall,
                    ..kept.clone()
                })
                .unwrap_or(entry)
        })
        .collect()
}

/// 目录换了代、预设没改（施工 O-2 中）：照旧快照的先后一件一件对现在的目录 `catalog`。提供者的照现在的登记，不再给这个会话
/// 的拿掉；自带的照旧快照里的原样，不新加。目录里没有了的：上一次对过的那一代里是提供者的拿掉（扩展关掉了、不再登记它），
/// 别的照旧留着、调到时暂时不可用（施工 4-2：程序升级拿掉的，载入的会话认不出来的；施工 F-5 中：随包卸掉的，调到时报已卸载）。
/// 新登记的提供者的工具加进来；新装上的内置包（上一次对过的那一代里这个包一件都没有，施工 F-5 中）的工具也加进来，照样过
/// 预设；快照照名字排（`miyu_policy` 的工具面）。
pub(super) fn followed(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: Option<&PresetFile>,
    reach: Reach,
) -> Vec<ToolEntry> {
    let fresh = offered(refresh, values, catalog, file, reach);
    let basis = refresh.seen.as_ref().map(|seen| &seen.catalog);
    let mut face = Vec::new();
    for kept in &refresh.snapshot.tools {
        match catalog.get(&kept.name) {
            Some(tool) if catalog.provided(&kept.name) => {
                let name = &tool.spec().name;
                face.extend(fresh.iter().find(|entry| entry.name == *name).cloned());
            }
            Some(_) => face.push(kept.clone()),
            // 随包卸掉的提供者的工具照旧留着、调到时报已卸载（施工 F-5 下）；关掉的扩展的拿掉。
            None if basis.is_some_and(|basis| basis.provided(&kept.name))
                && !catalog.gone(&kept.name) => {}
            None => face.push(kept.clone()),
        }
    }
    // 新装上的内置包：它的包在上一次对过的那一代里一件工具都没有。包里多了一件的（程序升级那种）照旧等预设改了才进来。
    let appeared = |name: &str| {
        basis.is_some_and(|basis| {
            catalog
                .package_of(name)
                .is_some_and(|package| !basis.packages().any(|owner| owner == package))
        })
    };
    let added: Vec<ToolEntry> = fresh
        .into_iter()
        .filter(|entry| {
            (catalog.provided(&entry.name) || appeared(&entry.name))
                && !face.iter().any(|kept| kept.name == entry.name)
        })
        .collect();
    face.extend(added);
    face
}
