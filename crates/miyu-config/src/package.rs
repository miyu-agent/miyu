//! 软件包清单的读法（施工 9-1 上，`docs/blueprint/packages.md`「清单的格式」，`05-内核接口.md` 第二节）：一份 TOML 读成
//! [`Manifest`]。只收图纸上的几张表、几个键，不认识的报错，免得写错了没人知道；报第一处，带代码、第几行、英文一句。
//! `[settings]` 的读法在 `settings.rs`（施工 9-1 下）。纯逻辑，不碰磁盘：在哪找、两层怎么认在 `miyu-store`。

use std::fmt;

use toml_edit::{Document, Item};

use crate::phrases::Phrases;

mod capability;
mod code;
mod command;
mod content;
mod credits;
mod features;
mod fields;
pub mod groups;
mod kinds;
mod links;
mod look;
mod mcp;
mod need;
mod reader;
mod search;
pub mod settings;
mod side;
mod slash;
mod target;
mod upgrade;
mod version;
mod widget;

pub use capability::Capability;
pub use code::Code;
pub use content::Content;
pub use features::Feature;
pub use links::{Connection, Worker};
pub use look::{LOGO_MAX_BYTES, LOGO_MAX_SIDE};
pub use mcp::{
    EnvValue, McpServer, Reach, TIMEOUT_DEFAULT as MCP_TIMEOUT_DEFAULT, personal as personal_mcp,
    venues_fit,
};
pub use need::Need;
pub use search::{SEARCH_SOURCE, SearchSource};
pub use settings::{Setting, SettingKind};
pub use side::{SIDE_TIERS, SideRequest};
pub use slash::{Access, SlashCommand};
pub use target::Target;
pub use upgrade::without_kind;
pub use widget::{MOST as WIDGETS_MOST, Place as WidgetPlace, Widget};

use reader::{Reader, line_of};

/// 读好的一份清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// 什么样的包：照带的表认（施工 F-8 上补，清单不写种类）。
    pub kind: PackageKind,
    /// 版本，给人看的字；没写的没有。
    pub version: Option<String>,
    /// 说得了的协议主版本 `[最低, 最高]`，和握手一样；吉祥物包不说协议，没有（施工 F-7）。
    pub protocol: Option<[u32; 2]>,
    /// 名字。
    pub name: Phrases,
    /// 一句说明；没写的是空的。
    pub summary: Phrases,
    /// 给 `miyu` 加的子命令（9-2 转交）：照写的先后，没写的是空的；一个包可以加几个（施工 F-13，`[[command]]`），第一个的程序
    /// 是这个包的程序（[`Manifest::command`]）。
    pub commands: Vec<Command>,
    /// 核心怎么拉起它（`kind = "process"`，9-4）。
    pub process: Option<Process>,
    /// 界面认的页、页面文件在哪（`kind = "ui"`）。
    pub ui: Option<Pages>,
    /// `miyu check` 怎么查它自己的文件（9-2 跑）。
    pub check: Option<Check>,
    /// 配置项（施工 9-1 下）：照写的先后；没有的是空的。
    pub settings: Vec<Setting>,
    /// 在哪几个组里（施工 BS-1 上中，设计 44 第三节第 2 条）：只有内置包能写，照写的先后；没写的是空的。原来的 `required`
    /// （必需的包）项目主人 2026-10-26 定删掉：出厂带什么是打包时挑的事。
    pub groups: Vec<String>,
    /// 写了的功能（施工 F-1）：没写 `[features]` 的没有，写了空表的是空的；算上照包算的那一个用 [`Manifest::features_of`]。
    pub features: Option<Vec<Feature>>,
    /// 平台接入（施工 F-1）：只有扩展包能写。
    pub connection: Option<Connection>,
    /// 依赖的包（施工 F-1，F-11 起什么种类都行）：缺了的装不上、装着的不起；包编号，照写的先后。
    pub depends: Vec<String>,
    /// 推荐的包（施工 F-1，F-11 起什么种类都行）：缺了照装、照起，少一部分本事。
    pub recommends: Vec<String>,
    /// 小程序怎么拉起（`kind = "worker"`，施工 F-1）。
    pub worker: Option<Worker>,
    /// 图标（施工 F-6 上）：Lucide 的图标名，只查过写法；没写的没有。
    pub icon: Option<String>,
    /// 包自带的图标（施工 F-18 上，`[package] logo`）：包目录里一张 PNG 或 WebP 的相对路径；有它的头用图，没有的照 `icon`。
    /// 只查写法，图本身在不在、合不合上限由核心查（`packages/status.rs` 的 `logo_problem`）。没写的没有。
    pub logo: Option<String>,
    /// 软件后台页（施工 F-6 上）：包目录里的子目录，入口是里面的 `index.html`；没写的没有。只有扩展、内置包能写。
    pub page: Option<String>,
    /// 吉祥物（`kind = "mascot"`，施工 F-7）：模型文件在包目录里的相对路径。
    pub mascot: Option<Mascot>,
    /// 终端的主题（`[theme]`，施工 F-16）：主题文件在包目录里的相对路径。只有主题包能写。
    pub theme: Option<Theme>,
    /// 带的 MCP 服务（施工 X-2，`[[mcp]]`）：照写的先后；没有的是空的。只有扩展包、`bundle` 能写。
    pub mcp: Vec<McpServer>,
    /// 声明的斜杠命令（施工 X-14 下，`[[slash]]`）：交给包的程序办；照写的先后，没有的是空的。只有扩展包能写。
    pub slash: Vec<SlashCommand>,
    /// 登记的旁路请求（施工 Q-3，`[[side]]`）：照写的先后，没有的是空的。只有扩展包能写。
    pub side: Vec<SideRequest>,
    /// 声明的搜索来源（施工 N-5，`[[search]]`）：照写的先后，没有的是空的。只有扩展包能写。
    pub search: Vec<SearchSource>,
    /// 界面块（施工 X-15 上，`[[widget]]`）：照写的先后，没有的是空的。只有扩展包能写。
    pub widgets: Vec<Widget>,
    /// 给别的软件调的方法（施工 X-18，`[export] methods`）：声明了 `packages.call` 的扩展经 `package.call` 只调得到这几个。只有
    /// 扩展包能写，没写的是空的。
    pub export: Vec<String>,
    /// 给别的包带的内容（施工 X-22，`[content.<包>] dir`）：照包的编号排；只有 `bundle`、扩展包能写，没写的是空的。
    pub content: Vec<Content>,
    /// 要这个地方有什么才有用（施工 X-25，`[package] needs`）：照 [`Need`] 的先后，没写的是空的。
    pub needs: Vec<Need>,
    /// 能装在哪几种系统上（施工 G-1，`[package] targets`）：照 [`Target::ALL`] 的先后；没写的是空的，哪种都能装。
    pub targets: Vec<Target>,
    /// 作者（施工 F-12，`[package] author`）：没写的没有。
    pub author: Option<String>,
    /// 上游：这个包的源码仓库，`https://` 开头（施工 F-12，`[package] upstream`）：没写的没有。
    pub upstream: Option<String>,
    /// 默认成员也能用（施工 IM-1 上，`[package] members`）：系统装上时照它，管理员在系统那一层改了的照他写的。只认出厂、系统
    /// 那一层的清单。没写的是不能。
    pub members: bool,
}

/// 吉祥物包的 `[mascot]`（施工 F-7，`packages.md`「吉祥物包」）：模型文件的格式由终端定，核心只认它在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mascot {
    /// 模型文件：包目录里的相对路径，例如 `mascot.json`。
    pub model: String,
}

/// 主题包的 `[theme]`（施工 F-16，`packages.md`「主题包」）：主题文件的格式由终端定，核心只认它在哪，同吉祥物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// 主题文件：包目录里的相对路径，例如 `theme.json`。
    pub file: String,
}

/// 包的种类：只说它跑在哪（设计 `30-插件框架.md` 第二节）；施工 F-8 上补起照带的表认（`kinds::of`），清单不写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// 界面：有人看着用，自己起来、经本机套接字连核心。
    Ui,
    /// 核心拉起、经标准输入输出说协议的扩展（9-4），通讯平台的接入也是这一种。
    Process,
    /// 内置（施工 F-1）：代码编在核心里，装的是清单和它的资源。
    Builtin,
    /// 小程序（施工 F-1）：核心按需拉起、空闲退出，说它自己的协议。
    Worker,
    /// 吉祥物（施工 F-7）：只有数据，终端照它画；不说协议、不跑程序、没有开关和配置项。
    Mascot,
    /// 终端的主题（施工 F-16）：同吉祥物只有数据，终端照它画框线、记号、留白；选哪个是终端自己的配置项。
    Theme,
    /// 一组东西、不带程序（施工 X-2，设计 `31-软件包.md` 第二节第 1 条）：MCP 服务（`[[mcp]]`）、技能、脚本（包文件夹里的
    /// `skills/`、`scripts/`，施工 X-13 上）。不说协议；带了服务的开关同扩展，默认关着；不带服务的没有开关（技能、脚本照预设开关）。
    Bundle,
}

impl PackageKind {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Ui => "ui",
            PackageKind::Process => "process",
            PackageKind::Builtin => "builtin",
            PackageKind::Worker => "worker",
            PackageKind::Mascot => "mascot",
            PackageKind::Theme => "theme",
            PackageKind::Bundle => "bundle",
        }
    }
}

/// `[command]`，`[[command]]` 的一个（施工 F-13）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// `miyu` 后面敲的那个词。
    pub name: String,
    /// 程序名，不带路径。
    pub program: String,
    /// 转交时加在人敲的参数前面的（施工 F-13）：没写的是空的。
    pub args: Vec<String>,
    /// 一句说明，`miyu -h` 照它列。
    pub about: Phrases,
    /// `name` 写在第几行：两个包撞了名，报在这一行。
    pub line: Option<usize>,
}

/// `[process]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    /// 拉起时带的参数。
    pub args: Vec<String>,
    /// 什么时候拉起。
    pub start: Start,
    /// 要哪些扩展能力（施工 9-4 下上）：照 [`Capability::ALL`] 的先后，不重复；没写的是空的。
    pub capabilities: Vec<Capability>,
    /// 有没有自己的系统账号（施工 O-4 下，`06-多用户与身份.md` U14）：有的，账号名就是包的编号，核心起来时建它，拉起的扩展
    /// 以它的身份连进来。没写的是没有。
    pub system_account: bool,
    /// 一份还是每人一份（施工 IM-1 上，接入通讯平台设计 06 第 4.6 节第 1 样）：没写的是一份。
    pub scope: Scope,
}

/// 带进程的包起几份（施工 IM-1 上）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// 一份，用包自己的账号跑（默认）。
    #[default]
    System,
    /// 每个用它的账号一份，以那个人的身份跑，数据放在他自己的家目录里。不能和 `system_account` 一起写。
    Account,
}

impl Scope {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::System => "system",
            Scope::Account => "account",
        }
    }
}

/// 什么时候拉起。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// 开关打开才拉起（默认）。
    Manual,
    /// 核心起来就拉起。
    Always,
}

impl Start {
    /// 清单、协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Start::Manual => "manual",
            Start::Always => "always",
        }
    }
}

/// `[ui]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages {
    /// 认的页（`--page`）。
    pub opens: Vec<String>,
    /// 页面文件的目录，相对资源目录；没有的没有。
    pub pages_dir: Option<String>,
}

/// `[check]`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// 跑 `<program> <args…>`。
    pub args: Vec<String>,
}

/// 读不成：第几行（整份的没有）、代码、换进给人看的那一句的那一格、英文一句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// 第几行，从 1 数；整份的（缺了 `[package]`）没有。
    pub line: Option<usize>,
    /// 代码。
    pub code: Code,
    /// 换进给人看的那一句的那一格：哪个键、写错的值。
    pub detail: String,
    /// 英文一句，协议和运行日志用。
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// 照正在跑的这一版 Miyu `running` 读一份清单（施工 X-31）：清单 `[package] miyu` 要更新的 Miyu 的，先报 `needs_newer_miyu`，
/// 不再往下读（不然新版本加的格在这里都是不认识的键）；给得了的照 [`read`]。装包、读装了的包用它；打包、软件源收录不比，用
/// [`read`]。
///
/// # Errors
///
/// 要更新的 Miyu；别的同 [`read`]。
pub fn read_for(text: &str, running: &str) -> Result<Manifest, Problem> {
    version::required(text, running)?;
    read(text)
}

/// 读一份清单，不比 `[package] miyu`（只查它的写法）。
///
/// # Errors
///
/// 读不成 TOML、少了必写的、写错了的，交回第一处。
pub fn read(text: &str) -> Result<Manifest, Problem> {
    let document = Document::parse(text).map_err(|error| Problem {
        line: error.span().map(|span| line_of(text, span.start)),
        code: Code::Syntax,
        detail: error.message().trim().to_string(),
        message: error.message().trim().to_string(),
    })?;
    let reader = Reader { text };
    let root = document.as_table();
    for (key, item) in root.iter() {
        if !fields::known_table(key) {
            return Err(reader.problem(
                Some(item),
                Code::UnknownTable,
                key,
                format!("unknown table [{key}]"),
            ));
        }
        // `[[mcp]]`、`[[slash]]`、`[[widget]]`、`[[side]]` 是一串表（施工 X-2、X-14 下、X-15 上、Q-3）：写成别的样子的由各自的模块
        // 报。`command` 一张、一串都行（施工 F-13）。
        if !item.is_table_like()
            && !(matches!(
                key,
                "mcp" | "slash" | "search" | "widget" | "side" | "command"
            ) && item.is_array_of_tables())
        {
            return Err(reader.problem(
                Some(item),
                Code::NotATable,
                key,
                format!("{key} must be a table"),
            ));
        }
    }
    let Some(package) = root.get("package").and_then(Item::as_table_like) else {
        return Err(reader.problem(
            None,
            Code::MissingKey,
            "package",
            "the [package] table is missing".to_string(),
        ));
    };
    let kind = kinds::of(&reader, root)?;
    let head = reader.package(package, &root["package"], kind)?;
    for (key, item) in root.iter() {
        reader.belongs(kind, key, item)?;
    }
    let commands = command::read(&reader, root.get("command"))?;
    let command = commands.first();
    let process = match root.get("process").and_then(Item::as_table_like) {
        Some(table) => {
            reader.needs_command(command, "process", &root["process"])?;
            Some(reader.process(table)?)
        }
        None => None,
    };
    let ui = match root.get("ui").and_then(Item::as_table_like) {
        Some(table) => Some(reader.pages(table)?),
        None => None,
    };
    let check = match root.get("check").and_then(Item::as_table_like) {
        Some(table) => {
            reader.needs_command(command, "check", &root["check"])?;
            reader.only(table, "check", fields::CHECK)?;
            Some(Check {
                args: reader.texts(table, "check", "args")?,
            })
        }
        None => None,
    };
    let settings = match root.get("settings") {
        Some(node) => settings::read(&reader, node)?,
        None => Vec::new(),
    };
    let features = match root.get("features") {
        Some(node) => Some(features::read(&reader, node)?),
        None => None,
    };
    let connection = match root.get("connection").and_then(Item::as_table_like) {
        Some(table) => Some(links::connection(&reader, table, &root["connection"])?),
        None => None,
    };
    let [depends, recommends] = ["depends", "recommends"].map(|name| {
        root.get(name)
            .and_then(Item::as_table_like)
            .map_or(Ok(Vec::new()), |table| {
                links::dependencies(&reader, table, name)
            })
    });
    let worker = match root.get("worker").and_then(Item::as_table_like) {
        Some(table) => Some(links::worker(&reader, table, &root["worker"])?),
        None => None,
    };
    if let Some(table) = root.get("builtin").and_then(Item::as_table_like) {
        reader.only(table, "builtin", &[])?;
    }
    let page = match root.get("page").and_then(Item::as_table_like) {
        Some(table) => Some(look::page(&reader, table, &root["page"])?),
        None => None,
    };
    let mascot = match root.get("mascot").and_then(Item::as_table_like) {
        Some(table) => Some(look::mascot(&reader, table, &root["mascot"])?),
        None => None,
    };
    let theme = match root.get("theme").and_then(Item::as_table_like) {
        Some(table) => Some(look::theme(&reader, table, &root["theme"])?),
        None => None,
    };
    let mcp = match root.get("mcp") {
        Some(node) => mcp::read(&reader, node)?,
        None => Vec::new(),
    };
    let slash = match root.get("slash") {
        Some(node) => slash::read(&reader, node)?,
        None => Vec::new(),
    };
    let side = match root.get("side") {
        Some(node) => side::read(&reader, node)?,
        None => Vec::new(),
    };
    let capabilities = process
        .as_ref()
        .map_or(&[][..], |process| process.capabilities.as_slice());
    slash::needs_capability(&slash, capabilities)?;
    let search = match root.get("search") {
        Some(node) => search::read(&reader, node)?,
        None => Vec::new(),
    };
    search::needs_capability(&search, capabilities)?;
    let widgets = match root.get("widget") {
        Some(node) => widget::read(&reader, node)?,
        None => Vec::new(),
    };
    widget::check(&widgets, capabilities, &slash)?;
    let export = match root.get("export").and_then(Item::as_table_like) {
        Some(table) => links::export(&reader, table)?,
        None => Vec::new(),
    };
    let content = match root.get("content") {
        Some(node) => content::read(&reader, node)?,
        None => Vec::new(),
    };
    Ok(Manifest {
        kind,
        version: head.version,
        protocol: head.protocol,
        name: head.name,
        summary: head.summary,
        commands,
        process,
        ui,
        check,
        settings,
        groups: head.groups,
        features,
        connection,
        depends: depends?,
        recommends: recommends?,
        worker,
        icon: head.icon,
        logo: head.logo,
        page,
        mascot,
        theme,
        mcp,
        slash,
        side,
        search,
        widgets,
        export,
        content,
        needs: head.needs,
        targets: head.targets,
        author: head.credits.author,
        upstream: head.credits.upstream,
        members: head.members,
    })
}

#[cfg(test)]
mod tests;
