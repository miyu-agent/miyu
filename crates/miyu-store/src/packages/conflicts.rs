//! 装之前查冲突（施工 F-8 六补，设计 `31-软件包.md` 第四节、定了的 I，`docs/blueprint/packages.md`「冲突」）：照 pacman 的
//! 「冲突的文件」，要装的包带的东西和装着的别的包重了名，就不装。每个包只往自己的文件夹里放，文件撞不上，撞的是几块共用的
//! 名字：子命令、功能的编号、工具名（功能下列的、脚本）、技能、MCP 服务、知识库。
//!
//! 以前装完重读清单才认撞没撞，谁先到照编号算：编号排在前的新包装得上，被判撞了的是装着的那个，悄悄失效。现在动文件以前
//! 比，新来的让。同一个编号（升级）不跟自己比；家目录里自己放的技能、脚本不算（自己的赢，设计 31 第五节第 3 条）。
//!
//! 碰磁盘（读包文件夹里的技能、脚本、库），调的一方放在阻塞线程里。

use std::collections::BTreeSet;
use std::path::Path;

use miyu_config::package::Manifest;

use super::Found;

/// 撞的是哪一样。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// 子命令名（`miyu <名字>`）。
    Command,
    /// 功能的编号（MCP 服务的另算 [`Kind::Mcp`]）。
    Feature,
    /// 工具名：功能下列的、脚本的。
    Tool,
    /// 技能名。
    Skill,
    /// MCP 服务名。
    Mcp,
    /// 知识库的编号。
    Knowledge,
}

impl Kind {
    /// 协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Command => "command",
            Kind::Feature => "feature",
            Kind::Tool => "tool",
            Kind::Skill => "skill",
            Kind::Mcp => "mcp",
            Kind::Knowledge => "knowledge",
        }
    }
}

/// 一处冲突：哪一样、什么名字、装着的哪个包已经有了。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Conflict {
    /// 哪一样。
    pub kind: Kind,
    /// 名字。
    pub name: String,
    /// 已经有它的包的编号。
    pub package: String,
}

/// 编号是 `id`、清单是 `manifest`、文件夹是 `dir` 的包，和装着的 `installed`（读成了的，同编号的不算）重了名的那几处，照
/// 哪一样、名字、包排。没有的是空的。
pub fn conflicts(id: &str, manifest: &Manifest, dir: &Path, installed: &[Found]) -> Vec<Conflict> {
    let mine = names(id, manifest, dir);
    let mut found = BTreeSet::new();
    for other in installed.iter().filter(|other| other.id != id) {
        let Ok(theirs) = &other.read else {
            continue;
        };
        let theirs = names(&other.id, theirs, &other.files_dir());
        for (kind, name) in mine.intersection(&theirs) {
            found.insert(Conflict {
                kind: *kind,
                name: name.clone(),
                package: other.id.clone(),
            });
        }
    }
    found.into_iter().collect()
}

/// 一个包占的名字。
fn names(id: &str, manifest: &Manifest, dir: &Path) -> BTreeSet<(Kind, String)> {
    let mut names = BTreeSet::new();
    if let Some(command) = &manifest.command {
        names.insert((Kind::Command, command.name.clone()));
    }
    for feature in manifest.features_of(id) {
        match feature.service {
            Some(service) => names.insert((Kind::Mcp, service)),
            None => names.insert((Kind::Feature, feature.id)),
        };
        names.extend(feature.tools.into_iter().map(|tool| (Kind::Tool, tool)));
    }
    names.extend(
        crate::scripts::carried(dir)
            .0
            .into_iter()
            .map(|name| (Kind::Tool, name)),
    );
    names.extend(
        crate::skills::carried(dir)
            .0
            .into_iter()
            .map(|name| (Kind::Skill, name)),
    );
    names.extend(
        crate::knowledge::carried(dir)
            .into_iter()
            .map(|name| (Kind::Knowledge, name)),
    );
    names
}

#[cfg(test)]
mod tests;
