//! `preset.get` 的一个功能读好的样子（施工 FG-1 从 `features.rs` 拆出来）：同一份既写成原来的 `features` 一项，也照设计 52 第三节
//! 分进 `sections`（`sections.rs`）。

use serde_json::{Value, json};

use miyu_policy::preset::By;

/// 一个功能是哪一类：照它分进哪一组。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kind {
    /// 包自己的功能、扩展替人交的功能：照包分组。
    Package(String),
    /// 技能；模板命令、扩展声明的命令也归这一组（设计 52 没写，施工 FG-1 先这样，报告里列着）。
    Skill,
    /// 脚本。
    Script,
    /// MCP 服务：配置里的、包带的都是。
    Mcp,
    /// 预设里写了、这台机器上没装的：只在原来的 `features` 里列。
    Missing,
}

/// 一件工具：编号、显示名、开不开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Tool {
    pub(super) name: String,
    pub(super) label: String,
    pub(super) on: bool,
}

/// 一个功能。
#[derive(Debug, Clone)]
pub(super) struct Entry {
    pub(super) id: String,
    /// 名字，照连接的语言挑好的。
    pub(super) name: String,
    pub(super) summary: Option<String>,
    /// 原来那一格 `package`：没有的不写。
    pub(super) package: Option<String>,
    pub(super) on: bool,
    pub(super) by: By,
    pub(super) installed: bool,
    pub(super) tools: Vec<Tool>,
    /// 清单里这个功能写的工具的先后（`[features.<编号>] tools`）：`sections` 照它排工具，没写到的接在后面；原来的 `features`
    /// 照旧照目录的先后。
    pub(super) tool_order: Vec<String>,
    pub(super) kind: Kind,
}

impl Entry {
    /// 装着的一个功能，没说明、没工具；`package` 是空的不写。
    pub(super) fn installed(
        (id, name): (String, String),
        package: &str,
        (on, by): (bool, By),
        kind: Kind,
    ) -> Entry {
        Entry {
            id,
            name,
            summary: None,
            package: (!package.is_empty()).then(|| package.to_string()),
            on,
            by,
            installed: true,
            tools: Vec::new(),
            tool_order: Vec::new(),
            kind,
        }
    }

    /// 带上说明。
    #[must_use]
    pub(super) fn summed(mut self, summary: Option<String>) -> Entry {
        self.summary = summary;
        self
    }

    /// 带上工具。
    #[must_use]
    pub(super) fn with_tools(mut self, tools: Vec<Tool>) -> Entry {
        self.tools = tools;
        self
    }

    /// 带上清单写的工具先后。
    #[must_use]
    pub(super) fn ordered(mut self, order: Vec<String>) -> Entry {
        self.tool_order = order;
        self
    }

    /// 工具照清单写的先后排（稳定，没写到的接在后面）。
    pub(super) fn tools_in_order(&self) -> Vec<&Tool> {
        let mut tools: Vec<&Tool> = self.tools.iter().collect();
        let rank = |tool: &&Tool| {
            self.tool_order
                .iter()
                .position(|name| *name == tool.name)
                .unwrap_or(usize::MAX)
        };
        tools.sort_by_key(rank);
        tools
    }

    /// 写成原来 `features` 的一项：格子、写法一个不动（`by` 照 [`By::as_flat_str`]，组算 `preset`）。
    pub(super) fn listed(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| json!({"name": tool.name, "label": tool.label, "on": tool.on}))
            .collect();
        let mut one = json!({
            "id": self.id,
            "name": self.name,
            "on": self.on,
            "by": self.by.as_flat_str(),
            "installed": self.installed,
            "tools": tools,
        });
        if let Some(package) = &self.package {
            one["package"] = json!(package);
        }
        if let Some(summary) = &self.summary {
            one["summary"] = json!(summary);
        }
        one
    }
}
