//! 管理员事先放行出沙盒的工具（施工 X-36，`11-权限与沙盒.md` A15，2026-10-20 项目主人定）：系统配置的
//! `sandbox.tools.<工具名> = "<来处>"`。工具自己声明的一律不认，只认管理员在系统配置里写的。
//!
//! 值写来处，仿 sudoers 写命令的完整路径：和工具目录里这件工具这时归的包编号一样才算放行（[`miyu_tool::Catalog::package_of`]：
//! 自己放的脚本是 `~home`，出厂的 `shell` 是 `basesystem`，装的包是它的编号）。脚本删了、别的包带了同名的工具来，对不上，
//! 不算。
//!
//! 回合开始照这一轮的配置读一份，交给权限策略和执行器（同技能文件夹的换法）：权限策略照它不问，执行器照它不给沙盒的规格。
//! 只碰得到自己工作区的会话、只读那一级照旧拒（`guard.rs`）。

use std::collections::BTreeMap;

use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value, Values, key};
use miyu_tool::Shelf;

/// 一件工具那一项的样子。工具名那一段照 `<name>`（同 `clicall.tools.<name>`）：MCP 工具的全名常超过 `<id>` 的 32 个字。
pub const TOOL: &str = "sandbox.tools.<name>";

/// 登记的一项：最后一段是工具名，`settings!` 写不了，手写。只认系统配置：管理员才碰得到。设置页先不画（隐私与安全页的
/// 画法是头那边的事），写进系统配置就生效。
pub const ITEMS: &[Item] = &[Item {
    key: TOOL,
    kind: Kind::Text { max: 200 },
    default: None,
    layers: &[Layer::System],
    tighten: None,
    env: None,
    applies: Applies::NextTurn,
    ui: Ui {
        // 和沙盒强度同一组（施工 5-16 下补）：组的编号哪一页都只算一回，挂在别的页上会把「隐私与安全」的沙盒组挤掉。
        page: "privacy",
        group: "sandbox",
        package: None,
        common: false,
        control: Control::Text,
        hidden: true,
    },
}];

/// 这一轮放行了的：工具名到写的来处。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unsandboxed(BTreeMap<String, String>);

impl Unsandboxed {
    /// 照这一轮的配置 `values` 读：真的键照样子拆开（名字那一段带了引号的也认得）。
    pub fn of(values: &Values) -> Unsandboxed {
        let mut granted = BTreeMap::new();
        for real in values.keys() {
            let Some(Value::Text(source)) = values.get(real) else {
                continue;
            };
            let Some(segments) = key::split(real) else {
                continue;
            };
            if let key::Fit::Yes(names) = key::fit(TOOL, &segments) {
                granted.extend(names.into_iter().map(|name| (name, source.to_string())));
            }
        }
        Unsandboxed(granted)
    }

    /// 工具 `name` 放行了：写了它，写的来处和架子 `shelf` 上这件工具现在归的包一样，它的文件这时不是链接（施工 X-37：链接能
    /// 指到她改得了的工作区，等于改了一件在沙盒外跑的工具）。
    pub fn granted(&self, name: &str, shelf: &Shelf) -> bool {
        self.0
            .get(name)
            .is_some_and(|source| shelf.package_of(name).as_deref() == Some(source.as_str()))
            && !shelf.get(name).is_some_and(|tool| tool.linked())
    }

    /// 写了的每一条：工具名、写的来处，照工具名排（`miyu check` 照它列）。
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, source)| (name.as_str(), source.as_str()))
    }
}

#[cfg(test)]
mod tests;
