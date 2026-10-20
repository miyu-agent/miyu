//! 技能、脚本、MCP 服务的全局开关在配置里的那一项（施工 X-27，`docs/blueprint/presets.md`「全局开关」，2026-10-20 项目主人定）：
//! `features.<功能编号>`，系统配置、个人设置都能写，`unlisted` 开着的预设照它，预设里明写了的照预设（`miyu_policy::preset` 的
//! `switches.rs`）。设置页不画：技能、脚本、MCP 那三页另给写法（`feature.set`）。
//!
//! 造会话、回合开始找预设时照这一轮的配置读（[`of`]），放进叠好的预设文件：改了从下一个回合开始生效。只认三类功能编号
//! （`miyu_policy::preset::switchable`），别的写了也不算。

use std::collections::BTreeMap;

use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value, Values, key};
use miyu_policy::preset::switchable;

/// 那一项的样子：最后一段是功能编号。
pub const FEATURE: &str = "features.<name>";

/// 登记的一项：最后一段是人起的名字，`settings!` 写不了，手写。
pub const ITEMS: &[Item] = &[Item {
    key: FEATURE,
    kind: Kind::Bool,
    default: None,
    layers: &[Layer::System, Layer::Personal],
    tighten: None,
    env: None,
    applies: Applies::NextTurn,
    ui: Ui {
        page: "general",
        group: "features",
        package: None,
        common: false,
        control: Control::Toggle,
        hidden: true,
    },
}];

/// 这一轮配置 `values` 里写了的全局开关：功能编号到开不开，只留三类功能的。
pub fn of(values: &Values) -> BTreeMap<String, bool> {
    let mut switches = BTreeMap::new();
    for real in values.keys() {
        let Some(Value::Bool(on)) = values.get(real) else {
            continue;
        };
        let Some(segments) = key::split(real) else {
            continue;
        };
        if let key::Fit::Yes(names) = key::fit(FEATURE, &segments) {
            switches.extend(
                names
                    .into_iter()
                    .filter(|feature| switchable(feature))
                    .map(|feature| (feature, *on)),
            );
        }
    }
    switches
}

/// 功能 `feature` 的全局开关这时开不开：没写的算开（`script.list`、`skill.list`、`mcp.list` 的 `on`）。
pub fn on(switches: &BTreeMap<String, bool>, feature: &str) -> bool {
    switches.get(feature).copied().unwrap_or(true)
}

#[cfg(test)]
mod tests;
