//! 一个包带了哪几样（软件源索引每一条的 `provides`，设计 `37-软件源.md`）：种类到名字。收录写索引（[`crate::collect`]）、核心答
//! `package.list` 每一项（施工 PV-2：「已安装」里也分得出技能、脚本）都调 [`of`]，同一份规则、同一个形状。
//!
//! - 清单里写的：子命令（`command`）、功能（`features`，只算清单写明的，照包算的那一个不算）、MCP 服务（`mcp`）、给哪几个包带了
//!   内容（`content`，照被带的包的编号，施工 G-9）、斜杠命令（`slash`）。
//! - 包文件夹里放的：技能（`skills/<名>/SKILL.md` 在的）、脚本（`scripts/` 下那一层的文件）、人格（`personas/` 下的目录）、预设
//!   （`presets/` 下的 `.toml`，去掉后缀）、知识库（`knowledge`，施工 PV-3：`kb/` 下编号写法对的目录，`kb.toml` 可以没有，
//!   同核心认包里的库，`docs/blueprint/knowledge.md` K-8、`miyu_store::knowledge::carried`）。
//!
//! 每一种照名字排、去重；空的种类不写，什么都没带的是空表。碰磁盘，读不了的目录当没有。

use std::collections::BTreeMap;
use std::path::Path;

use miyu_config::package::Manifest;

/// 清单是 `manifest`、包文件夹是 `folder` 的包带了哪几样：种类到名字，见模块说明。
pub fn of(manifest: &Manifest, folder: &Path) -> BTreeMap<String, Vec<String>> {
    let mut provides = BTreeMap::new();
    let mut put = |kind: &str, mut names: Vec<String>| {
        names.sort();
        names.dedup();
        if !names.is_empty() {
            provides.insert(kind.to_string(), names);
        }
    };
    put(
        "command",
        manifest.commands.iter().map(|c| c.name.clone()).collect(),
    );
    put(
        "features",
        manifest
            .features
            .iter()
            .flatten()
            .map(|feature| feature.id.clone())
            .collect(),
    );
    put(
        "mcp",
        manifest
            .mcp
            .iter()
            .map(|server| server.name.clone())
            .collect(),
    );
    put(
        "content",
        manifest
            .content
            .iter()
            .map(|content| content.target.clone())
            .collect(),
    );
    put(
        "slash",
        manifest
            .slash
            .iter()
            .map(|slash| slash.name.clone())
            .collect(),
    );
    put(
        "skills",
        listed(&folder.join("skills"), |path| {
            path.join("SKILL.md").is_file()
        }),
    );
    put("scripts", listed(&folder.join("scripts"), Path::is_file));
    put("personas", listed(&folder.join("personas"), Path::is_dir));
    put(
        "presets",
        listed(&folder.join("presets"), Path::is_file)
            .into_iter()
            .filter_map(|name| name.strip_suffix(".toml").map(str::to_string))
            .collect(),
    );
    // 不跟链接，同核心认包里的库（`DirEntry::file_type`）：指到目录的链接不算一个库。
    put(
        "knowledge",
        listed(&folder.join("kb"), |path| {
            path.symlink_metadata().is_ok_and(|meta| meta.is_dir())
        })
        .into_iter()
        .filter(|id| crate::index::plain_id(id))
        .collect(),
    );
    provides
}

/// 目录 `dir` 里照 `keep` 留下的名字；没有这个目录的是空的。
fn listed(dir: &Path, keep: impl Fn(&Path) -> bool) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| keep(&entry.path()))
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect()
}
