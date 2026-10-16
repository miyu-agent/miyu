//! 包之间的依赖（施工 F-11，设计 31 第三节，`docs/blueprint/packages.md`「依赖」）：清单的 `[depends]` 写的是别的包的编号，什么
//! 种类都行，照 pacman：缺了的装不上；装着的被人弄缺了（手删、升级以后多依赖了一个）的不起、`package.list`、`miyu check` 报出来；
//! 被别人依赖着的卸不掉。`[recommends]` 缺了照装、照起。读成了的清单才算装着。

use miyu_config::package::Manifest;
use miyu_store::packages::Found;

/// 清单 `manifest` 依赖、在 `installed` 里没有读成了的那几个：照写的先后。
pub(crate) fn missing<'a>(installed: &[&Found], manifest: &'a Manifest) -> Vec<&'a str> {
    manifest
        .depends
        .iter()
        .filter(|id| {
            !installed
                .iter()
                .any(|one| one.id == **id && one.read.is_ok())
        })
        .map(String::as_str)
        .collect()
}

/// `installed` 里依赖包 `id` 的那几个（读成了的，不算它自己）：照编号。
pub(crate) fn dependents(installed: &[Found], id: &str) -> Vec<String> {
    let mut by: Vec<String> = installed
        .iter()
        .filter(|one| one.id != id)
        .filter(|one| {
            one.read
                .as_ref()
                .is_ok_and(|manifest| manifest.depends.iter().any(|dep| dep == id))
        })
        .map(|one| one.id.clone())
        .collect();
    by.sort();
    by
}

#[cfg(test)]
mod tests;
