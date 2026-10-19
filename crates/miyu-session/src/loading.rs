//! 工具要不要先加载，人在个人设置里改的那一层（施工 L-3，设计 `25-工具加载.md` W9，2026-10-20 项目主人定）：来处写的（说明
//! 文件、脚本开头、扩展登记、MCP 服务表）是默认，`loading.tools.<工具名>`、`loading.mcp.<服务名>` 写了的盖过它。一件工具先照
//! 它自己那一项，没写的是 MCP 的工具再照它的服务那一项，都没写的照来处。
//!
//! 造工具面时照这一轮的配置算（`agents.rs` 的 `Offers`）：改了从下一个回合开始生效，记成一次有原因的前缀改写，
//! 和工具面的别的变化一样。键里的名字照配置的写法（小写字母开头，小写字母、数字、`-`、`_`）：写不进键的工具名（例如带大写的
//! MCP 工具）只能照服务那一项改。

use std::collections::BTreeMap;

use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value, Values};

/// 一件工具的那一项。
const TOOL: &str = "loading.tools.";

/// 一个 MCP 服务的那一项。
const MCP: &str = "loading.mcp.";

/// 登记的两项：最后一段是人起的名字，`settings!` 写不了，手写。设置页不画：技能、脚本、MCP 那几页另给写法。
pub const ITEMS: &[Item] = &[item("loading.tools.<id>"), item("loading.mcp.<id>")];

const fn item(key: &'static str) -> Item {
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
            group: "loading",
            package: None,
            common: false,
            control: Control::Toggle,
            hidden: true,
        },
    }
}

/// 这一轮配置里写了的那几项：键（去掉前缀以后的名字）到要不要先加载。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Loading {
    tools: BTreeMap<String, bool>,
    servers: BTreeMap<String, bool>,
}

impl Loading {
    /// 照这一轮的配置 `values` 读。
    pub(crate) fn of(values: &Values) -> Loading {
        let mut loading = Loading::default();
        for key in values.keys() {
            let Some(Value::Bool(set)) = values.get(key) else {
                continue;
            };
            if let Some(tool) = key.strip_prefix(TOOL) {
                loading.tools.insert(tool.to_string(), *set);
            } else if let Some(server) = key.strip_prefix(MCP) {
                loading.servers.insert(server.to_string(), *set);
            }
        }
        loading
    }

    /// 工具 `name` 要不要先加载：它自己那一项，MCP 的工具再照服务那一项，都没写的照来处写的 `registered`。
    pub(crate) fn need_load(&self, name: &str, registered: bool) -> bool {
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
