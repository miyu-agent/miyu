//! `preset.get` 的一个个功能（施工 F-3 下，设计 `30-插件框架.md` 第四节，`docs/blueprint/presets.md`「怎么走」第 8 条）：
//! 装了的功能照清单读的先后，每个带开不开、下面的每件工具开不开；接着是登记上的脚本（施工 X-12 中）、用得上的技能（施工
//! X-11 上）、模板命令（施工 X-14 上），一个一个功能；扩展替这个人交的（施工 IM-6）接在装了的包后面；预设里写了、没装的接在后面。头照它一个功能一个开关画，展开能逐件关。

use std::collections::BTreeSet;

use miyu_config::Words;
use miyu_policy::preset::PresetFile;
use miyu_policy::script::feature_id as script_feature;
use miyu_policy::skill::feature_id;
use miyu_policy::slash::feature_id as command_feature;
use miyu_store::human::Human;
use miyu_store::presets::Found;
use miyu_store::scripts::HOME_PACKAGE;

use super::Carried;
use super::entry::{Entry, Kind, Tool};
use crate::Core;
use crate::config::methods::words;
use crate::personas::pick;

/// 一个个功能：
/// - 装了的：`{"id", "name", "summary", "package", "on", "by", "installed": true, "tools"}`，名字、说明照它的清单、照语言挑，没说明的没有
///   `summary`；`on` 是叠好以后开不开，`by` 是谁定的（施工 X-27）：`preset` 预设里写了，`global` 照技能、脚本、MCP 三页的
///   全局开关，`default` 都没写、照 `unlisted`；`tools` 是归它的、工具目录里现在有的工具，照名字排，每件
///   `{"name", "label", "on"}`：`label` 照给人看的字里这件工具的显示名，没有的照工具名；功能关着的都是关着，开着的照
///   `[tools]` 关没关。`package` 是它归哪个包（施工 ENV-1：头照它按包分组，包名照 `package.list` 查），同预设 `[software]`
///   认的那个编号：你自己放的脚本、MCP 服务、模板命令是 `~home`；不归包的技能没有这一格。
/// - 预设的 `[features]`、`[software]` 里写了、这台机器上没装的：`{"id", "name", "on", "by", "installed": false, "tools": []}`，
///   照编号排；名字照给人看的字 `software/<编号>`（以前内置的几样），没有的是编号。写的是装了的包的编号的不另列（它的功能
///   已经列了）。
pub(super) fn listed(core: &Core, found: &Found, language: &str, carried: &Carried) -> Vec<Entry> {
    let words = words(core, language).ok();
    let catalog = core.tools.current();
    let found_packages = core.packages_of();
    let table = crate::scripts::features(core);
    let file = &found.file.clone().following(super::switches(core));
    let row = |on: bool| {
        let (words, catalog) = (words.as_ref(), &catalog);
        move |name: &str| Tool {
            name: name.to_string(),
            label: label(words, catalog, name, language),
            on: on && !file.tools_off.contains(name),
        }
    };
    let mut listed = Vec::new();
    let mut packages = BTreeSet::new();
    // 程序不在的扩展当没装（施工 F-6 上）：它的功能照「写了没装」列。
    for (package, manifest) in found_packages
        .iter()
        .filter(|one| !crate::packages::absent(one))
        .filter_map(|one| Some((one.id.as_str(), one.read.as_ref().ok()?)))
    {
        packages.insert(package);
        // 被配置里同名的服务盖住的不列：那个功能列在配置的那一个上（施工 F-9 下）。
        for feature in manifest.features_of(package).into_iter().filter(|feature| {
            feature
                .service
                .as_ref()
                .is_none_or(|service| !core.home_mcp.covers(service))
        }) {
            let decided = file.decided(&feature.id, package);
            let tools = tools_of(&catalog, &table, (package, &feature.id), row(decided.0));
            // 包带的 MCP 服务归 MCP 那一组（设计 52 第一节第 3 条：这三类一定各自独立）。
            let kind = match feature.service {
                Some(_) => Kind::Mcp,
                None => Kind::Package(package.to_string()),
            };
            let name = pick(&feature.name, language).unwrap_or_default();
            listed.push(
                Entry::installed((feature.id, name), package, decided, kind)
                    .summed(pick(&feature.summary, language))
                    .with_tools(tools)
                    .ordered(feature.tools),
            );
        }
    }
    // 扩展替这个人交的功能（施工 IM-6，`feature.offer`）：归交的那个包，写法同包自己的功能。
    for offer in core.extensions.offers.of() {
        let (id, package) = (&offer.feature.id, offer.feature.package.as_str());
        let decided = file.decided(id, package);
        let tools = tools_of(&catalog, &table, (package, id), row(decided.0));
        let name = pick(&offer.name, language).unwrap_or_else(|| id.clone());
        listed.push(
            Entry::installed(
                (id.clone(), name),
                package,
                decided,
                Kind::Package(package.into()),
            )
            .summed(pick(&offer.summary, language))
            .with_tools(tools),
        );
    }
    let scripts = core.scripts_for();
    // 脚本各算一个功能（施工 X-12 中）：名字、说明照脚本写给人看的几格挑这个连接的语言，没写名字的照工具名；它自己那件工具
    // 带着同一个名字，开不开照功能和 `[tools]`。
    for script in &scripts {
        let id = script_feature(&script.head.name);
        let package = crate::scripts::package(script);
        let decided = file.decided(&id, package);
        let name =
            pick(&script.head.display_name, language).unwrap_or_else(|| script.head.name.clone());
        let tool = Tool {
            name: script.head.name.clone(),
            label: name.clone(),
            on: decided.0 && !file.tools_off.contains(&script.head.name),
        };
        listed.push(
            Entry::installed((id, name), package, decided, Kind::Script)
                .summed(pick(&script.head.summary, language))
                .with_tools(vec![tool]),
        );
    }
    // 配置里的 MCP 服务各算一个功能（施工 F-9 下）：名字是服务名；工具是目录里挂在 `~home` 下、归它的。
    for server in core.home_mcp.now().iter() {
        let id = server.feature_id();
        let decided = file.decided(&id, HOME_PACKAGE);
        let tools = tools_of(&catalog, &table, (HOME_PACKAGE, &id), row(decided.0));
        listed.push(
            Entry::installed((id, server.name.clone()), HOME_PACKAGE, decided, Kind::Mcp)
                .with_tools(tools),
        );
    }
    let mut carried_ids = BTreeSet::new();
    listed.extend(carried_entries(file, language, carried, &mut carried_ids));
    // `[software]` 里写的组名（施工 BS-1 上中，`basesystem = true`）不是没装的包：它照组分给了组里的包，不另列。
    let groups: BTreeSet<&String> = file.groups.values().flatten().collect();
    let written: BTreeSet<&String> = file
        .features
        .keys()
        .chain(file.software.keys())
        .filter(|id| {
            !table.installed(id)
                && !packages.contains(id.as_str())
                && !carried_ids.contains(*id)
                && !groups.contains(id)
        })
        .collect();
    for id in written {
        let name = words
            .as_ref()
            .and_then(|words| Words::sentence(words, &format!("software/{id}"), &[]))
            .unwrap_or_else(|| id.clone());
        let mut entry =
            Entry::installed((id.clone(), name), "", file.decided(id, id), Kind::Missing);
        entry.installed = false;
        listed.push(entry);
    }
    listed
}

/// 技能（施工 X-11 上）、模板命令（施工 X-14 上）、扩展声明的命令（施工 X-14 下）各算一个功能，没有工具；编号记进 `ids`。
/// - 技能：名字、说明照技能写给人看的两格挑这个连接的语言（施工 X-13 下），没写名字的照技能的名字；不归包的没有 `package`。
/// - 模板命令：名字是 `/<命令名>`，说明照模板写的；自己放的挂在 `~home` 下。
/// - 扩展声明的命令：名字是 `/<命令名>`，说明照清单的，挂在它的包下；自己放的同名模板盖住它的不列。
fn carried_entries(
    file: &PresetFile,
    language: &str,
    carried: &Carried,
    ids: &mut BTreeSet<String>,
) -> Vec<Entry> {
    let mut listed = Vec::new();
    for skill in &carried.skills {
        let id = feature_id(&skill.head.name);
        let package = skill.package.as_deref().unwrap_or_default();
        let decided = file.decided(&id, package);
        let name =
            pick(&skill.head.display_name, language).unwrap_or_else(|| skill.head.name.clone());
        listed.push(
            Entry::installed((id, name), package, decided, Kind::Skill)
                .summed(pick(&skill.head.summary, language)),
        );
    }
    for command in &carried.commands {
        let id = command_feature(&command.name);
        let package = command.package.as_deref().unwrap_or(HOME_PACKAGE);
        let decided = file.decided(&id, package);
        let name = format!("/{}", command.name);
        listed.push(
            Entry::installed((id, name), package, decided, Kind::Skill)
                .summed(pick(&command.template.summary, language)),
        );
    }
    for one in carried
        .declared
        .iter()
        .filter(|one| !carried.commands.iter().any(|t| t.name == one.command.name))
    {
        let id = one.command.feature_id();
        let decided = file.decided(&id, &one.package);
        let name = format!("/{}", one.command.name);
        listed.push(
            Entry::installed((id, name), &one.package, decided, Kind::Skill)
                .summed(pick(&one.command.summary, language)),
        );
    }
    ids.extend(listed.iter().map(|entry| entry.id.clone()));
    listed
}

/// 工具目录里挂在包 `package` 下、归功能 `feature` 的工具，照目录的先后，每件照 `row` 写。
fn tools_of(
    catalog: &miyu_tool::Catalog,
    table: &miyu_policy::features::Features,
    (package, feature): (&str, &str),
    row: impl Fn(&str) -> Tool,
) -> Vec<Tool> {
    catalog
        .specs()
        .map(|spec| spec.name.as_str())
        .filter(|name| {
            catalog.package_of(name) == Some(package)
                && table.of_tool(package, name) == Some(feature)
        })
        .map(row)
        .collect()
}

/// 工具 `name` 给人看的显示名：给人看的字里有的照它；工具自己登记了的（MCP 服务给的 `title`、扩展、脚本写的）照连接的语言
/// `language` 挑；都没有的，MCP 的去掉 `mcp__<服务名>__` 只留工具自己的名字（施工 X-27 再补，网页报的），别的照工具名。
fn label(
    words: Option<&Human>,
    catalog: &miyu_tool::Catalog,
    name: &str,
    language: &str,
) -> String {
    if let Some(face) = words.and_then(|words| words.tool(name)) {
        return face.name.clone();
    }
    if let Some(picked) = catalog
        .get(name)
        .and_then(|tool| pick(&tool.display_name(), language))
    {
        return picked;
    }
    name.strip_prefix(miyu_tool::MCP_PREFIX)
        .and_then(|rest| rest.split_once("__"))
        .map_or_else(|| name.to_string(), |(_, tool)| tool.to_string())
}
