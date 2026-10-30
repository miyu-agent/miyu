//! 没装的命令是哪个包带的（施工 CL-7）：`miyu web`、`miyu tui` 这类命令由软件包加，没装时敲了是「未知命令」。照本机存着的
//! 软件源索引找哪个包提供这个命令（索引的 `provides.command`），找到了提示去装它。
//!
//! 只是提示：不连核心、不去网上拿，本机存着的索引不验签名（谁改了也只是提示错一个包名，装的时候核心照旧验）。官方的
//! （`state/packages/.sync/`）先找，再照名字找配置里另加的源（`state/packages/.sources/<名字>/`）。

use std::path::Path;

use serde_json::Value;

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

/// 本机存着的软件源索引里提供命令 `name` 的包的编号；没有存过索引、哪个包都不带它的没有。
pub fn offering(name: &str) -> Option<String> {
    let root = DataRoot::locate(&Env::current()).ok()?;
    offering_under(&root.state().join("packages"), name)
}

/// 同 [`offering`]，索引在 `packages`（数据根的 `state/packages/`）下面找。
pub(crate) fn offering_under(packages: &Path, name: &str) -> Option<String> {
    let mut indexes = vec![packages.join(".sync").join("index.json")];
    if let Ok(entries) = std::fs::read_dir(packages.join(".sources")) {
        let mut sources: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path().join("index.json"))
            .collect();
        sources.sort();
        indexes.extend(sources);
    }
    indexes.iter().find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        offered_in(&serde_json::from_str(&text).ok()?, name)
    })
}

/// 一份索引 `index` 里提供命令 `name` 的包：照索引里的先后，第一个。
fn offered_in(index: &Value, name: &str) -> Option<String> {
    index["packages"].as_array()?.iter().find_map(|entry| {
        let commands = entry["provides"]["command"].as_array()?;
        commands
            .iter()
            .any(|command| command.as_str() == Some(name))
            .then(|| entry["id"].as_str().map(str::to_string))
            .flatten()
    })
}

#[cfg(test)]
mod tests;
