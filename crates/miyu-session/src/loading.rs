//! 工具要不要先加载，人在个人设置里改的那一层（施工 L-3，设计 `25-工具加载.md` W9，2026-10-20 项目主人定）：来处写的（说明
//! 文件、脚本开头、扩展登记、MCP 服务表）是默认，`loading.tools.<工具名>`、`loading.mcp.<服务名>` 写了的盖过它。一件工具先照
//! 它自己那一项，没写的是 MCP 的工具再照它的服务那一项，都没写的照来处。
//!
//! 造工具面时照这一轮的配置算（`agents.rs` 的 `Offers`）：改了从下一个回合开始生效，记成一次有原因的前缀改写，
//! 和工具面的别的变化一样。工具名那一段是占位 `<name>`（施工 X-27：MCP 工具的全名常超过 `<id>` 的 32 个字），大写、长名字都
//! 写得进；服务名照 `<id>`。头的脚本、MCP 两页照 [`Loading::tool`]、[`Loading::server`] 说是来处定的还是人改的。
//!
//! 脚本里能不能调（`clicall`，施工 M-2，`39-脚本调工具.md` 第三节，2026-10-20 项目主人定）照同一个办法：`clicall.tools.<name>`、
//! `clicall.mcp.<id>` 盖过来处写的，读法、先后都一样（[`Loading::clicall_of`]）。

use std::collections::BTreeMap;

use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value, Values, key};

/// 一件工具那一项的样子。
pub const TOOL: &str = "loading.tools.<name>";

/// 一个 MCP 服务那一项的样子。
pub const MCP: &str = "loading.mcp.<id>";

/// 脚本里能不能调，一件工具那一项的样子（施工 M-2）。
pub const CLICALL_TOOL: &str = "clicall.tools.<name>";

/// 脚本里能不能调，一个 MCP 服务那一项的样子（施工 M-2）。
pub const CLICALL_MCP: &str = "clicall.mcp.<id>";

/// 登记的几项：最后一段是人起的名字，`settings!` 写不了，手写。设置页不画：脚本、MCP 那两页另给写法（`tool.set_loading`，
/// 施工 X-27）；`clicall` 的两项头上还没有写法，写进个人设置就生效（施工 M-2）。
pub const ITEMS: &[Item] = &[
    item(TOOL, "loading"),
    item(MCP, "loading"),
    item(CLICALL_TOOL, "clicall"),
    item(CLICALL_MCP, "clicall"),
];

const fn item(key: &'static str, group: &'static str) -> Item {
    Item {
        key,
        kind: Kind::Bool,
        default: None,
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: Ui {
            page: "general",
            group,
            package: None,
            common: false,
            control: Control::Toggle,
            hidden: true,
        },
    }
}

/// 这一轮配置里写了的那几项：键里的名字到要不要先加载（[`Loading::of`]），或者脚本里能不能调（[`Loading::clicall_of`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loading {
    tools: BTreeMap<String, bool>,
    servers: BTreeMap<String, bool>,
}

impl Loading {
    /// 照这一轮的配置 `values` 读要不要先加载：真的键照样子拆开（名字那一段带了引号的也认得）。
    pub fn of(values: &Values) -> Loading {
        Loading::read(values, TOOL, MCP)
    }

    /// 同 [`Loading::of`]，读的是脚本里能不能调的那两项（施工 M-2）。
    pub fn clicall_of(values: &Values) -> Loading {
        Loading::read(values, CLICALL_TOOL, CLICALL_MCP)
    }

    fn read(values: &Values, tool: &'static str, mcp: &'static str) -> Loading {
        let mut loading = Loading::default();
        for real in values.keys() {
            let Some(Value::Bool(set)) = values.get(real) else {
                continue;
            };
            let Some(segments) = key::split(real) else {
                continue;
            };
            if let key::Fit::Yes(names) = key::fit(tool, &segments) {
                loading
                    .tools
                    .extend(names.into_iter().map(|name| (name, *set)));
            } else if let key::Fit::Yes(names) = key::fit(mcp, &segments) {
                loading
                    .servers
                    .extend(names.into_iter().map(|name| (name, *set)));
            }
        }
        loading
    }

    /// 工具 `name` 自己那一项：没写的没有。
    pub fn tool(&self, name: &str) -> Option<bool> {
        self.tools.get(name).copied()
    }

    /// MCP 服务 `server` 那一项：没写的没有。
    pub fn server(&self, server: &str) -> Option<bool> {
        self.servers.get(server).copied()
    }

    /// 工具 `name` 要不要先加载（或者脚本里能不能调）：它自己那一项，MCP 的工具再照服务那一项，都没写的照来处写的
    /// `registered`。
    pub fn need_load(&self, name: &str, registered: bool) -> bool {
        self.pick(name, registered)
    }

    /// 同 [`Loading::need_load`]，给 [`Loading::clicall_of`] 读出来的那一份用，读着顺。
    pub fn clicall(&self, name: &str, registered: bool) -> bool {
        self.pick(name, registered)
    }

    fn pick(&self, name: &str, registered: bool) -> bool {
        self.tools
            .get(name)
            .or_else(|| server_of(name).and_then(|server| self.servers.get(server)))
            .copied()
            .unwrap_or(registered)
    }
}

/// MCP 工具的服务名：目录里的名字是 `mcp__<服务名>__<工具名>`。不是 MCP 的没有。
fn server_of(name: &str) -> Option<&str> {
    name.strip_prefix("mcp__")?
        .split_once("__")
        .map(|(server, _)| server)
}

#[cfg(test)]
mod tests;
