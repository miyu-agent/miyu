//! 每个人把某个包对自己整个关掉（施工 U23 末上，设计 06 U23，项目主人 2026-10-24）：个人设置一项 `packages.<编号>.off`，只能写
//! 个人层；写在系统配置里的不认（系统那一层的总闸照旧是 `package.disable`）。
//!
//! 设置页上每个装着的包那一组底下一个开关：给每个包现拼一项（[`item_for`]，挂在「软件包」页、那个包自己那一组）；带占位的那一项
//! （[`ITEMS`]，不上设置页）接住没装的编号，先关后装就是关着的（`miyu_config::key::item_of` 先认具体的键、再认样子）。必需的包
//! 不拼，也不认（[`of`] 交回的由用的一方再去掉必需的）。

use std::collections::BTreeSet;

pub use miyu_config::package::settings::OFF;
use miyu_config::package::settings::PAGE;
use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value, Values, key};

/// 带占位的那一项：接住没装的编号，不上设置页（挂在「通用」页的功能那一组，同 `features.<name>`）。
pub const ITEMS: &[Item] = &[item(OFF, ("general", "features"), None, true)];

/// 包 `id` 自己的那一项：「软件包」页、这个包那一组。键、组名照编号造一次、一直用到退出（同包的配置项，
/// `miyu_config::package::settings::items`）。
pub fn item_for(id: &str) -> Item {
    let key: &'static str = Box::leak(key::fill(OFF, &[id]).into_boxed_str());
    let group: &'static str = Box::leak(id.to_string().into_boxed_str());
    item(key, (PAGE, group), Some(group), false)
}

/// 这一项：开关，只写个人层，下一个回合生效；挂在 `(页, 组)`。
const fn item(
    key: &'static str,
    (page, group): (&'static str, &'static str),
    package: Option<&'static str>,
    hidden: bool,
) -> Item {
    Item {
        key,
        kind: Kind::Bool,
        default: None,
        layers: &[Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: Ui {
            page,
            group,
            package,
            common: false,
            control: Control::Toggle,
            hidden,
        },
    }
}

/// 这一轮配置 `values` 里关了的包的编号（写成 `true` 的），照编号排。必需的包也在里面：用的一方照这一刻的清单去掉。
pub fn of(values: &Values) -> BTreeSet<String> {
    values
        .keys()
        .filter(|real| matches!(values.get(real), Some(Value::Bool(true))))
        .filter_map(|real| match key::fit(OFF, &key::split(real)?) {
            key::Fit::Yes(mut names) => names.pop(),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
