//! 软件包的「启用」（施工 UX-1，项目主人 2026-11-17）：设置页上每个有开关的包那一组底下一项 `packages.<编号>.enabled`，`true`
//! 是开。它不写在配置文件里：值就是这个包真的开没开（`package.list` 的 `enabled`），`config.set` 改它就是 `package.enable`、
//! `package.disable`（`miyu-endpoint` 的 `config/enabled.rs`）。所以它不进配置清单，配置文件里写了的照不认识的键警告；
//! `config.schema`、`config.get` 照这一刻装着的包现拼。
//!
//! 原来的 `packages.<编号>.off`（施工 U23 末上，多用户时「只对你自己隐藏这个包」）和真正的开关是两个开关管一件事，
//! 2026-11-17 删了，没发版、不留兼容。

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use super::settings::PAGE;
use crate::item::{Applies, Control, Item, Kind, Layer, Ui};
use crate::key;

/// 这一项的样子：中间一段是包的编号。
pub const KEY: &str = "packages.<id>.enabled";

/// 带占位的那一项：只给字表对账（每种语言都要有它的名字、说明），不进配置清单。
pub const ITEMS: &[Item] = &[item(KEY, ("packages", "features"), None)];

/// 拼过的每个包的那一项：键、组名照编号造一次、一直用到退出（同包的配置项，[`super::settings::items`]）。包的个数有界。
static MADE: Mutex<BTreeMap<String, &'static Item>> = Mutex::new(BTreeMap::new());

/// 包 `id` 的那一项：「软件包」页、这个包那一组。
pub fn item_for(id: &str) -> &'static Item {
    let mut made = MADE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(item) = made.get(id) {
        return item;
    }
    let key: &'static str = Box::leak(key::fill(KEY, &[id]).into_boxed_str());
    let group: &'static str = Box::leak(id.to_string().into_boxed_str());
    let item: &'static Item = Box::leak(Box::new(item(key, (PAGE, group), Some(group))));
    made.insert(id.to_string(), item);
    item
}

/// 真的键 `key` 是哪个包的「启用」：交回包的编号；不是这一项的没有。
pub fn id_of(key: &str) -> Option<String> {
    match key::fit(KEY, &key::split(key)?) {
        key::Fit::Yes(mut names) => names.pop(),
        _ => None,
    }
}

/// 这一项：开关，当场生效，只在系统这一层；挂在 `(页, 组)`。
const fn item(
    key: &'static str,
    (page, group): (&'static str, &'static str),
    package: Option<&'static str>,
) -> Item {
    Item {
        key,
        kind: Kind::Bool,
        default: None,
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::Now,
        ui: Ui {
            page,
            group,
            package,
            common: false,
            control: Control::Toggle,
            hidden: false,
        },
    }
}

#[cfg(test)]
mod tests;
