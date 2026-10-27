//! 自己装的盖过系统的（施工 U23 下上，设计 06 第八又补第 5 条）：账号自己那一层（[`Layer::User`]）和出厂的、系统那一层同编号的，
//! 他看到的是他自己那一份；系统那一份带程序的（[`runs_program`]）不盖：程序、包账号、扩展是整台机器一份的，他那份照旧报
//! `duplicate`。自己那一层只收不带程序的包（端点装的时候挡）。

use std::path::Path;

use miyu_config::package::{Manifest, PackageKind};

use super::{Found, Layer, Packages};

/// 包带不带程序（施工 U23 下上）：界面、扩展、内置、小程序这几种，写了子命令、`[process]`、`[worker]`、MCP 服务（网址的也算：
/// 成员自己的 MCP 等 MM-1）的。技能、脚本、预设、人格、主题、吉祥物这些只有数据的不算。
pub fn runs_program(manifest: &Manifest) -> bool {
    let kind = match manifest.kind {
        PackageKind::Ui | PackageKind::Process | PackageKind::Builtin | PackageKind::Worker => true,
        PackageKind::Mascot | PackageKind::Theme | PackageKind::Bundle => false,
    };
    kind || !manifest.commands.is_empty()
        || manifest.process.is_some()
        || manifest.worker.is_some()
        || !manifest.mcp.is_empty()
}

/// 照编号、层排好的 `found` 里，被他自己那一层盖过的出厂的、系统那一层的那几份去掉。读不成的系统那一份当带程序（不盖）。
pub(super) fn overridden(found: &mut Vec<Found>) {
    let own: Vec<String> = found
        .iter()
        .filter(|one| one.layer == Layer::User)
        .map(|one| one.id.clone())
        .collect();
    found.retain(|one| {
        let plain = one
            .read
            .as_ref()
            .is_ok_and(|manifest| !runs_program(manifest));
        one.layer == Layer::User || !own.contains(&one.id) || !plain
    });
}

impl Packages {
    /// 账号自己装的那一层的目录（施工 U23 下上：自己装、卸只动它）；不是 [`Packages::of_account`] 造的、他那层就是系统层的没有。
    pub fn own_dir(&self) -> Option<&Path> {
        self.dirs
            .iter()
            .find(|(layer, _)| *layer == Layer::User)
            .map(|(_, dir)| dir.as_path())
    }
}
