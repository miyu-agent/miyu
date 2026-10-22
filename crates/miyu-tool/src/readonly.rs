//! 只禁写的名单可配（施工 5-17 下，`docs/designs/11-权限与沙盒.md` 第四节「只禁写」）：`sandbox.readonly.add`（人加的）、
//! `sandbox.readonly.remove`（去掉的出厂项）。同私密目录（[`crate::private`]）记两份、不记整份，「隐私与安全」页自成一组，排在
//! 私密目录后面，没有总开关（网页 2026-10-22 画好的形状）。
//!
//! 「去掉的出厂项」能写的就是这台机器的出厂清单（[`miyu_fs::SHIPPED_READONLY`]）：清单里登记成文字的列表，出厂清单由端点在
//! `config.schema` 里当选项交给头、`config.set` 时查，同私密目录。权限策略照这一轮的配置拼（[`list`]）。

use std::path::Path;

use miyu_config::{Applies, Control, Item, Kind, Layer, Setting, Ui, Values};
use miyu_fs::Readonly;

/// 人加的：`~/…`、绝对路径，或者工作区里的相对路径。
pub const ADD: &str = "sandbox.readonly.add";

/// 去掉的出厂项。
pub const REMOVE: &str = "sandbox.readonly.remove";

/// 一项最多几个字。
const LONGEST: usize = 4096;

/// 设置页「隐私与安全」那一页、只禁写那一组。
const UI: Ui = Ui {
    page: "privacy",
    group: "readonly",
    package: None,
    common: false,
    control: Control::List,
    hidden: false,
};

/// 这两项：系统、个人两层，下一轮生效；没写的是空的。
pub const ITEMS: &[Item] = &[
    Item {
        key: ADD,
        kind: Kind::List(&Kind::Text { max: LONGEST }),
        default: None,
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: UI,
    },
    Item {
        key: REMOVE,
        kind: Kind::List(&Kind::Text { max: LONGEST }),
        default: None,
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: UI,
    },
];

/// 照最终值 `values` 拼出这一刻的只禁写名单：出厂的去掉人去掉的，加上人加的（[`Readonly::with`]）。家目录 `home` 不知道的，
/// `~` 开头的不算。
pub fn list(values: &Values, home: Option<&Path>) -> Readonly {
    let read = |key: &str| <Option<Vec<String>>>::read(values.get(key)).unwrap_or_default();
    Readonly::with(home, &read(ADD), &read(REMOVE))
}

#[cfg(test)]
mod tests;
