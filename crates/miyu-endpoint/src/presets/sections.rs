//! `preset.get` 的 `sections`（施工 FG-1，设计 `52-功能分组.md` 第一节、第三节，`docs/blueprint/presets.md`「功能分组」）：核心把
//! 功能分好组、排好先后，头照着画，不翻译、不排序、不认编号。
//!
//! - 先是出厂的组，照组表 `resources/packages/groups.toml` 的先后（`kind: "group"`）：组里的功能照组的 `order`（包的先后），包里
//!   照清单写的先后；包照清单的 `[package] groups` 进组。
//! - 再是没进组的扩展包：带几个功能的各自一组（`kind: "package"`，`id` 是包编号，名字是包名，照包编号排）；只带一个功能的合进
//!   「扩展」一组（`kind: "extensions"`，一个扩展一行，`name` 是包名，照包编号排；一个都没有的不列）。
//! - 最后是技能、脚本、MCP 服务各一组（`kind` 和 `id` 都是 `skills`、`scripts`、`mcp`），没有的也列。包带的技能、脚本、MCP
//!   服务也归这三组，不归它的包所在的组。命令（模板、扩展声明的）跟技能一组。写了没装的不进哪一组。
//! - 一项 `{kind: "feature", id, name, on, by}`；带几件工具的另带 `tools: [{id, name, on}]`，一件的不带。`by` 分出 `group`
//!   （[`miyu_policy::preset::By::Group`]）。同一组里名字一样的，后面括号带包名。`on`、`total` 是这一组开着的、全部的功能几个。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_config::package::groups::Group;

use super::entry::{Entry, Kind};
use crate::Core;
use crate::config::methods::words;
use crate::personas::pick;

/// 「扩展」那一组的编号，也是它的 `kind`。
const EXTENSIONS: &str = "extensions";

/// 末尾三组：`kind`（也是编号），给人看的字的键里的名字，没有字时的英文名。
const TAIL: [(&str, &str); 3] = [
    ("skills", "Skills"),
    ("scripts", "Scripts"),
    ("mcp", "MCP servers"),
];

/// 分组要知道的：组表，装着的包的名字（照连接的语言挑好的）和组，几个固定组的名字。
pub(super) struct Shelf {
    groups: Vec<Group>,
    language: String,
    packages: BTreeMap<String, Shelved>,
    /// `skills`、`scripts`、`mcp`、`extensions` 的名字。
    names: BTreeMap<&'static str, String>,
}

/// 一个装着的包：名字、在哪几个组里。
struct Shelved {
    name: String,
    groups: Vec<String>,
}

impl Shelf {
    /// 照核心这时装着的包、出厂的组表、连接的语言 `language` 拼。
    pub(super) fn of(core: &Core, language: &str) -> Shelf {
        let packages = core
            .system_packages()
            .iter()
            .filter_map(|one| {
                let manifest = one.read.as_ref().ok()?;
                let name = pick(&manifest.name, language).unwrap_or_else(|| one.id.clone());
                let groups = manifest.groups.clone();
                Some((one.id.clone(), Shelved { name, groups }))
            })
            .collect();
        let said = words(core, language).ok();
        let named = |kind: &'static str, fallback: &str| {
            let name = said
                .as_ref()
                .and_then(|words| Words::sentence(words, &format!("preset-sections/{kind}"), &[]))
                .unwrap_or_else(|| fallback.to_string());
            (kind, name)
        };
        let names = TAIL
            .into_iter()
            .chain([(EXTENSIONS, "Extensions")])
            .map(|(kind, fallback)| named(kind, fallback))
            .collect();
        Shelf {
            groups: crate::packages::group::table(core),
            language: language.to_string(),
            packages,
            names,
        }
    }

    /// 包 `package` 归的出厂组：清单写的组里头一个在组表里的。
    fn group_of(&self, package: &str) -> Option<&str> {
        let shelved = self.packages.get(package)?;
        shelved
            .groups
            .iter()
            .find(|group| self.groups.iter().any(|one| &one.id == *group))
            .map(String::as_str)
    }

    /// 包 `package` 的名字：没读到清单的照编号。
    fn package_name(&self, package: &str) -> String {
        self.packages
            .get(package)
            .map_or_else(|| package.to_string(), |shelved| shelved.name.clone())
    }
}

/// 分好的一组组，照设计 52 第三节写。
pub(super) fn sections(entries: &[Entry], shelf: &Shelf) -> Vec<Value> {
    let mut sections = Vec::new();
    for group in &shelf.groups {
        let mut members: Vec<(&Entry, &str)> = entries
            .iter()
            .filter_map(|entry| match &entry.kind {
                Kind::Package(package) if shelf.group_of(package) == Some(&group.id) => {
                    Some((entry, package.as_str()))
                }
                _ => None,
            })
            .collect();
        let rank = |package: &str| group.order.iter().position(|one| one == package);
        // 稳定排序：同一个包里照清单写的先后。`order` 没写到的排在后面、照编号。
        members.sort_by(|(_, a), (_, b)| {
            let key = |package: &str| (rank(package).unwrap_or(usize::MAX), package.to_string());
            key(a).cmp(&key(b))
        });
        let rows = members
            .into_iter()
            .map(|(entry, _)| (entry, entry.name.clone()));
        let name = pick(&group.name, &shelf.language).unwrap_or_else(|| group.id.clone());
        sections.push(section(("group", &group.id, &name), rows.collect(), shelf));
    }
    // 没进组的扩展包，照包编号。
    let mut loose: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for entry in entries {
        if let Kind::Package(package) = &entry.kind
            && shelf.group_of(package).is_none()
        {
            loose.entry(package.as_str()).or_default().push(entry);
        }
    }
    let mut singles = Vec::new();
    for (package, members) in &loose {
        match members.as_slice() {
            [one] => singles.push((*one, shelf.package_name(package))),
            _ => {
                let rows = members.iter().map(|entry| (*entry, entry.name.clone()));
                let name = shelf.package_name(package);
                sections.push(section(("package", package, &name), rows.collect(), shelf));
            }
        }
    }
    if !singles.is_empty() {
        let name = &shelf.names[EXTENSIONS];
        sections.push(section((EXTENSIONS, EXTENSIONS, name), singles, shelf));
    }
    for (kind, _) in TAIL {
        let rows = entries
            .iter()
            .filter(|entry| tail_of(&entry.kind) == Some(kind))
            .map(|entry| (entry, entry.name.clone()));
        let name = &shelf.names[kind];
        sections.push(section((kind, kind, name), rows.collect(), shelf));
    }
    sections
}

/// 末尾三组里归哪一组：不归的是 `None`。
fn tail_of(kind: &Kind) -> Option<&'static str> {
    match kind {
        Kind::Skill => Some("skills"),
        Kind::Script => Some("scripts"),
        Kind::Mcp => Some("mcp"),
        Kind::Package(_) | Kind::Missing => None,
    }
}

/// 一组：`(kind, id, name)`，组里的每一行是功能和它这一行的名字。
fn section(
    (kind, id, name): (&str, &str, &str),
    rows: Vec<(&Entry, String)>,
    shelf: &Shelf,
) -> Value {
    let on = rows.iter().filter(|(entry, _)| entry.on).count();
    let total = rows.len();
    let items: Vec<Value> = rows
        .iter()
        .map(|(entry, shown)| {
            let twins = rows.iter().filter(|(_, other)| other == shown).count();
            let shown = match (twins > 1, told_apart(entry, shown, shelf)) {
                (true, Some(by)) => format!("{shown} ({by})"),
                _ => shown.clone(),
            };
            item(entry, shown)
        })
        .collect();
    json!({"kind": kind, "id": id, "name": name, "on": on, "total": total, "items": items})
}

/// 重名时括号里写的：功能的包名；包名就是这一行的名字的（「扩展」那一组）写包编号。不归装着的包的（你自己放的）不带。
fn told_apart(entry: &Entry, shown: &str, shelf: &Shelf) -> Option<String> {
    let package = entry.package.as_deref()?;
    let name = &shelf.packages.get(package)?.name;
    if name == shown {
        Some(package.to_string())
    } else {
        Some(name.clone())
    }
}

/// 一行功能。带几件工具的列出来，一件的不列（设计 52 第三节第 2 条）；照清单写的先后。
fn item(entry: &Entry, name: String) -> Value {
    let mut item = json!({
        "kind": "feature",
        "id": entry.id,
        "name": name,
        "on": entry.on,
        "by": entry.by.as_str(),
    });
    if entry.tools.len() > 1 {
        item["tools"] = entry
            .tools_in_order()
            .into_iter()
            .map(|tool| json!({"id": tool.name, "name": tool.label, "on": tool.on}))
            .collect();
    }
    item
}
