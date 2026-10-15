//! 扩展放在 system 尾部的那几段（施工 X-20，`docs/designs/34-表情包.md` 第五节第 5 条）：照这一刻的工具面挑出有工具在面上的
//! 包，它说过的照它说的，没说过的照快照里钉的；工具不在面上的包不带。

use std::collections::BTreeMap;

use miyu_policy::ToolEntry;
use miyu_tool::Catalog;

/// 这一轮要放的那几段：`pinned` 是快照里钉的，`asked` 是扩展这一刻说的（经端口问到的，`None` 是去掉了）。
pub(crate) fn resolve(
    catalog: &Catalog,
    face: &[ToolEntry],
    pinned: &BTreeMap<String, String>,
    asked: &BTreeMap<String, Option<String>>,
) -> BTreeMap<String, String> {
    face.iter()
        .filter_map(|tool| catalog.package_of(&tool.name))
        .filter_map(|package| {
            let text = match asked.get(package) {
                Some(said) => said.clone(),
                None => pinned.get(package).cloned(),
            };
            Some((package.to_string(), text?))
        })
        .collect()
}
