//! 清单格式的真相源（施工 Y-5 末上：冻结清单格式）：清单认的每一张表、每张表认的每一格只写在这里，读的各处照它挡，作者文档
//! `docs/authors/manifest.md`、`.en.md` 由测试拿来对它。冻结以后只加不改不删：加一格、一张表要写进这里和两份作者文档，用到新
//! 写法的包在 `[package] miyu` 写上要的最低版本；改名、删格、改意思的不做（要做就是新的协议版本）。

/// 一张表：名字、认的格、作者文档里有没有。读清单的各处直接用下面的常量挡，`keys`、`for_authors` 只给对作者文档的测试读。
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "认的格由各处直接用常量挡，这两格只给对作者文档的测试读"
    )
)]
pub(super) struct Table {
    /// 表名，`[[mcp]]` 这种数组表也写 `mcp`。
    pub(super) name: &'static str,
    /// 认的格：几组拼起来（配置项照类型多几格，分开写）。
    pub(super) keys: &'static [&'static [&'static str]],
    /// 给第三方作者用的：作者文档要写全；出厂包才用的不写：`builtin`（代码编在核心里）、`ui`（终端、网页这些界面）、`worker`
    /// （小程序，只找 `miyu` 旁边的程序）。
    pub(super) for_authors: bool,
}

/// `[package]`。
pub(super) const PACKAGE: &[&str] = &[
    "version", "protocol", "name", "summary", "prefix", "groups", "icon", "logo", "needs",
    "targets", "author", "upstream", "miyu",
];

/// `[command]`、`[[command]]`。
pub(super) const COMMAND: &[&str] = &["name", "program", "about", "args"];

/// `[process]`。
pub(super) const PROCESS: &[&str] = &["args", "start", "capabilities"];

/// `[ui]`：界面包。
pub(super) const UI: &[&str] = &["opens", "pages_dir"];

/// `[check]`。
pub(super) const CHECK: &[&str] = &["args"];

/// `[settings.<名字>]` 每一项都认的几格。
pub(super) const SETTING: &[&str] = &[
    "type",
    "default",
    "layers",
    "applies",
    "name",
    "description",
    "hidden",
];

/// `type = "int"` 多的几格。
pub(super) const SETTING_INT: &[&str] = &["min", "max", "step", "slider"];

/// `type = "number"` 多的几格（施工 PC-1）。
pub(super) const SETTING_NUMBER: &[&str] = &["min", "max", "step", "slider"];

/// `type = "option"` 多的几格。
pub(super) const SETTING_OPTION: &[&str] = &["choices"];

/// `type = "text"` 多的几格。
pub(super) const SETTING_TEXT: &[&str] = &["max", "lines"];

/// `type = "list"` 多的几格。
pub(super) const SETTING_LIST: &[&str] = &["element"];

/// `[features.<编号>]`。
pub(super) const FEATURE: &[&str] = &["name", "summary", "tools"];

/// `[connection]`。
pub(super) const CONNECTION: &[&str] = &["platform"];

/// `[depends]`、`[recommends]`：`workers` 是以前的写法，读进同一份。
pub(super) const LINKS: &[&str] = &["packages", "workers"];

/// `[worker]`。
pub(super) const WORKER: &[&str] = &["program", "args"];

/// `[page]`。
pub(super) const PAGE: &[&str] = &["dir"];

/// `[mascot]`。
pub(super) const MASCOT: &[&str] = &["model"];

/// `[theme]`。
pub(super) const THEME: &[&str] = &["file"];

/// 一个 MCP 服务除了名字的几格：清单的 `[[mcp]]`、系统配置里的服务都是这几格。
pub(super) const MCP_SERVER: [&str; 9] = [
    "command",
    "env",
    "url",
    "bearer",
    "headers",
    "venues",
    "timeout_ms",
    // 退役的（施工 L-1 登记、FL-1 拆掉：工具一律全量）：写了的照旧认得这个键，值不读。
    "need_load",
    "clicall",
];

/// `[[mcp]]`：名字加 [`MCP_SERVER`]。
pub(super) const MCP: [&str; 10] = [
    "name",
    MCP_SERVER[0],
    MCP_SERVER[1],
    MCP_SERVER[2],
    MCP_SERVER[3],
    MCP_SERVER[4],
    MCP_SERVER[5],
    MCP_SERVER[6],
    MCP_SERVER[7],
    MCP_SERVER[8],
];

/// `[[slash]]`。
pub(super) const SLASH: &[&str] = &["name", "summary", "argument", "access"];

/// `[[side]]`（施工 Q-3）。
pub(super) const SIDE: &[&str] = &["id", "tier", "name", "summary"];

/// `[[search]]`。
pub(super) const SEARCH: &[&str] = &["id", "name"];

/// `[[widget]]`。
pub(super) const WIDGET: &[&str] = &["id", "place", "title", "scope", "command", "state"];

/// `[export]`。
pub(super) const EXPORT: &[&str] = &["methods"];

/// `[content.<包的编号>]`。
pub(super) const CONTENT: &[&str] = &["dir"];

/// 清单认得的表，照这个先后；哪种包能写哪几张见 `kinds::tables`。
pub(super) const TABLES: [Table; 22] = [
    table("package", &[PACKAGE]),
    Table {
        name: "builtin",
        keys: &[],
        for_authors: false,
    },
    table("command", &[COMMAND]),
    table("process", &[PROCESS]),
    Table {
        name: "ui",
        keys: &[UI],
        for_authors: false,
    },
    table("check", &[CHECK]),
    table(
        "settings",
        &[
            SETTING,
            SETTING_INT,
            SETTING_NUMBER,
            SETTING_OPTION,
            SETTING_TEXT,
            SETTING_LIST,
        ],
    ),
    table("features", &[FEATURE]),
    table("connection", &[CONNECTION]),
    table("depends", &[LINKS]),
    table("recommends", &[LINKS]),
    Table {
        name: "worker",
        keys: &[WORKER],
        for_authors: false,
    },
    table("page", &[PAGE]),
    table("mascot", &[MASCOT]),
    table("theme", &[THEME]),
    table("mcp", &[&MCP]),
    table("slash", &[SLASH]),
    table("side", &[SIDE]),
    table("search", &[SEARCH]),
    table("widget", &[WIDGET]),
    table("export", &[EXPORT]),
    table("content", &[CONTENT]),
];

/// 给作者用的一张表。
const fn table(name: &'static str, keys: &'static [&'static [&'static str]]) -> Table {
    Table {
        name,
        keys,
        for_authors: true,
    }
}

/// 认不认得表 `name`。
pub(super) fn known_table(name: &str) -> bool {
    TABLES.iter().any(|table| table.name == name)
}

#[cfg(test)]
mod tests;
