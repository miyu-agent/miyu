//! 基础系统（`docs/designs/10-自带软件.md` 第三节）：随发行附带、几乎每个预设都打开的几件工具，软件包的编号
//! 是 `basesystem`。
//!
//! 工具的说明、参数格式、输出里给她看的几句都放在资源目录的 `software/basesystem/` 下（`26-提示词.md`
//! 第八节），核心起来时读。现在 [`tools`] 里有读的三件：`read`（施工 4-4 上）、`glob`、`grep`（施工 4-4 下），
//! 和写的 `write`（施工 4-6 上）、`edit`（施工 4-6 中）、`trash`（施工 4-6 下），执行命令的 `shell`（施工 4-8），
//! 翻这个会话自己记录的 `history`（施工 6-4，施工 C-4 多认别的会话），派子代理的 `subagent`（施工 7-5，7-5 再补从 `agent`
//! 改名），看、停派出去的任务的 `jobs`（施工 7-4），发话的 `send_message`（施工 7-7，施工 C-5 从 `message_agent` 改名，
//! 多发得到别的会话），列你别的主会话的 `sessions`（施工 C-3），查这个会话的用量、金额、上下文的 `session_usage`（施工 8-15），
//! 问人一组题、等回答的 `ask_user`（施工 D-2），维护这个会话的待办清单的 `todowrite`（施工 D-3），看软件包的 `packages`
//! （施工 F-10 上）。
//! 名字、参数、输出照成熟 harness 的规范，以 Claude Code 为主（`10-自带软件.md` 第十节）。

mod ask_user;
mod attach;
mod blocking;
mod common;
mod edit;
mod glob;
mod grep;
mod history;
mod jobs;
mod load;
mod package;
mod packages;
mod pattern;
mod read;
mod send_message;
mod session_usage;
mod sessions;
mod shell;
mod show;
mod subagent;
mod text;
mod todowrite;
mod trash;
mod walk;
mod write;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_tool::Tool;

pub use load::LoadError;

/// 一个脚本当一件工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」）：开头读好的 `head`、脚本文件 `file`，给她看的几句
/// 照资源目录 `resources` 里的字。`source` 是它的来处：包的编号，数据根里自己放的是 `~home`（施工 X-44：状态目录照它分）。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对，说是哪一份。
pub fn script(
    resources: &Path,
    head: &miyu_policy::script::ScriptHead,
    file: PathBuf,
    source: &str,
) -> Result<Arc<dyn Tool>, LoadError> {
    Ok(Arc::new(shell::script::Script::load(
        resources, head, file, source,
    )?))
}

/// 基础系统的每件工具：照资源目录 `resources` 里的字造好。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对，说是哪一份。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let common = common::Common::load(resources)?;
    Ok(vec![
        Arc::new(read::Read::load(resources, common.clone())?),
        Arc::new(glob::Glob::load(resources, common.clone())?),
        Arc::new(grep::Grep::load(resources, common.clone())?),
        Arc::new(write::Write::load(resources, common.clone())?),
        Arc::new(edit::Edit::load(resources, common.clone())?),
        Arc::new(trash::Trash::load(resources, common.clone())?),
        Arc::new(show::Show::load(resources, common.clone())?),
        Arc::new(shell::Shell::load(resources, common.clone())?),
        Arc::new(history::History::load(resources, common.clone())?),
        Arc::new(subagent::Subagent::load(resources, common.clone())?),
        Arc::new(jobs::Jobs::load(resources, common.clone())?),
        Arc::new(send_message::SendMessage::load(resources, common.clone())?),
        Arc::new(sessions::Sessions::load(resources, common.clone())?),
        Arc::new(session_usage::SessionUsage::load(resources)?),
        Arc::new(ask_user::AskUser::load(resources, common.clone())?),
        Arc::new(todowrite::TodoWrite::load(resources, common)?),
        Arc::new(packages::Packages::load(resources)?),
        Arc::new(package::Package::load(resources)?),
    ])
}

/// 基础系统拆成的内置包（施工 BS-1 上中，设计 44 第三节第 1 条）：编号照清单的先后，`miyu-base-project-docs`、`miyu-base-clicall`
/// 没有工具。施工 PN-3 起是 `miyu-base-*`（设计 48 第一节第 2 条），和工具名不再相同，工具归哪个包照 [`TOOLS`]。
pub const PACKAGES: [&str; 18] = [
    "miyu-base-read",
    "miyu-base-write",
    "miyu-base-edit",
    "miyu-base-glob",
    "miyu-base-grep",
    "miyu-base-trash",
    "miyu-base-shell",
    "miyu-base-jobs",
    "miyu-base-subagent",
    "miyu-base-ask-user",
    "miyu-base-todowrite",
    "miyu-base-history",
    "miyu-base-session-usage",
    "miyu-base-sessions",
    "miyu-base-packages",
    "miyu-base-show",
    "miyu-base-project-docs",
    "miyu-base-clicall",
];

/// 基础系统的组名（施工 BS-1 上中）：上面这些包的清单都写着它，出厂的组表 `resources/packages/groups.toml` 里有它。
pub const GROUP: &str = "basesystem";

/// 基础系统的每件工具归哪个包（施工 PN-3）：`sessions`、`send_message` 一个包，`packages`、`package` 一个包，别的一件一个包。
/// 和各包清单的 `[features] tools` 一致，出厂资源的测试守着。
pub const TOOLS: [(&str, &str); 18] = [
    ("read", "miyu-base-read"),
    ("write", "miyu-base-write"),
    ("edit", "miyu-base-edit"),
    ("glob", "miyu-base-glob"),
    ("grep", "miyu-base-grep"),
    ("trash", "miyu-base-trash"),
    ("shell", "miyu-base-shell"),
    ("jobs", "miyu-base-jobs"),
    ("subagent", "miyu-base-subagent"),
    ("ask_user", "miyu-base-ask-user"),
    ("todowrite", "miyu-base-todowrite"),
    ("history", "miyu-base-history"),
    ("session_usage", "miyu-base-session-usage"),
    ("sessions", "miyu-base-sessions"),
    ("send_message", "miyu-base-sessions"),
    ("packages", "miyu-base-packages"),
    ("package", "miyu-base-packages"),
    ("show", "miyu-base-show"),
];

/// 一件工具归哪个包（施工 BS-1 上中，施工 PN-3 起照 [`TOOLS`]）。不是基础系统的工具照 `basesystem` 算（[`grouped`] 给测试的假
/// 工具用）。
pub fn package_of(tool: &str) -> &'static str {
    TOOLS
        .iter()
        .find(|(name, _)| *name == tool)
        .map_or(GROUP, |(_, package)| package)
}

/// 包的编号和它的几件工具，照 [`PACKAGES`] 的先后（施工 BS-1 上中）。
pub type Packaged = Vec<(&'static str, Vec<Arc<dyn Tool>>)>;

/// 基础系统的工具照包分好（施工 BS-1 上中）：包的编号和它的几件，照 [`PACKAGES`] 的先后；没有工具的包不在里面。
///
/// # Errors
///
/// 同 [`tools`]。
pub fn packages(resources: &Path) -> Result<Packaged, LoadError> {
    Ok(grouped(tools(resources)?))
}

/// 几件工具照 [`package_of`] 分到基础系统的包里（施工 BS-1 上中），照 [`PACKAGES`] 的先后：核心登记时用，测试拿假工具造目录也用。
pub fn grouped(tools: Vec<Arc<dyn Tool>>) -> Packaged {
    let mut grouped: Packaged = Vec::new();
    for tool in tools {
        let package = package_of(&tool.spec().name);
        match grouped.iter_mut().find(|(id, _)| *id == package) {
            Some((_, tools)) => tools.push(tool),
            None => grouped.push((package, vec![tool])),
        }
    }
    grouped.sort_by_key(|(id, _)| PACKAGES.iter().position(|one| one == id));
    grouped
}
