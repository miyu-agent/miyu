//! 列出来却没登记的（施工 X-37，X-39 起也记提示词；从 `mcp.rs` 挪来）：看管那一格记着，`mcp.list`、软件包列表、`miyu check` 照它说。

/// 列出来却没登记的一件工具（施工 X-37）、一个提示词（施工 X-39）：服务那边的原名（没写名字的是空的），和为什么（`name_taken`
/// 名字被占了：截短、清理以后撞了，或者别的工具已经叫这个；`name` 没写名字、名字不合写法；`parameters` 参数格式不对）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Skipped {
    pub(crate) name: String,
    pub(crate) why: &'static str,
    /// 是提示词（施工 X-39）：`mcp.list` 写成 `prompt`，不是 `tool`。
    pub(crate) prompt: bool,
}

impl Skipped {
    /// 一件工具。
    pub(super) fn tool(name: String, why: &'static str) -> Skipped {
        Skipped {
            name,
            why,
            prompt: false,
        }
    }

    /// 一个提示词（施工 X-39）。
    pub(super) fn prompt(name: String, why: &'static str) -> Skipped {
        Skipped {
            name,
            why,
            prompt: true,
        }
    }

    /// 列的时候就认不出来的那一项。
    pub(super) fn unread(unread: &miyu_mcp::Unread) -> Skipped {
        match unread {
            miyu_mcp::Unread::Nameless(_) => Skipped::tool(String::new(), "name"),
            miyu_mcp::Unread::Parameters(name) => Skipped::tool(name.clone(), "parameters"),
        }
    }
}
