//! 脚本调工具的通道（施工 M-3，设计 `39-脚本调工具.md` 第五节；施工 XG-1 下从 `miyu-basesystem` 的 `shell/channel` 挪出来，设计
//! `63-扩展工具的效果和声明.md` 第六节第 2 条）：跑命令的工具（内置 `shell`，扩展声明了 `runs_commands` 的）拿到了端口
//! （[`miyu_tool::Call::nested`]）的，起命令前开一个只给这一次的端点，命令里的 `miyu call` 连上来调工具。
//!
//! - 端点：Unix 上一个 Unix 套接字，放在临时目录下一个核心进程一个、只有本人进得去的目录里（`0700`），名字随机；Windows 上是命名
//!   管道（`windows.rs`）。告诉子进程的是 `MIYU_CALL_SOCK`（路径、管道名）、`MIYU_CALL_TOKEN`（128 位随机的令牌）；`PATH` 最前面
//!   放一个只有 `miyu` 的目录，指向核心自己：脚本里调到的一定是同一个版本。`shell` 自己设进命令的环境（[`Channel::env`]），扩展
//!   照 `tool.call` 的 `channel` 设进它起的命令（[`Channel::handed`]）。
//! - 认人（[`Kin`]）两种，都要令牌对得上：`shell` 的照这条命令的进程组（Unix，对方的进程号经 `SO_PEERCRED` / `LOCAL_PEERPID`
//!   拿，查它的组）、作业对象（Windows）；扩展的照「连上来的是这个扩展进程的子孙」（`kin.rs`：Linux 照 `/proc` 的父进程链，macOS
//!   `proc_pidinfo`，Windows 照快照的父进程号）。过不了的回退出码 2。令牌漏了也只有这条命令（这个扩展）自己的进程树用得上。
//! - 一次连接一个请求、一个回答：4 字节大端长度加 UTF-8 JSON。请求帧超过 1 MiB 的断开；同时连着的超过 32 个，多的断开。
//! - 丢掉 [`Channel`] 就关：服务的任务掐掉、套接字删掉，令牌跟着作废，脚本里等着的 `miyu call` 读到结尾（退出码 3）。`shell` 前台
//!   的命令结束时丢，放到后台的跟着进程交给任务表、任务结束时丢（[`keep`]）；扩展的这次调用答了、超时、被掐时丢。
//!
//! 给她看的几句（`resources/core/call/`，原来在 `software/basesystem/shell/call/`，字节没变）：[`Texts`] 是通道答的，[`Section`]
//! 是说明末尾讲脚本调工具的那一段（`miyu_tool::Tool::clicall_section`）。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::template::Template;
use miyu_tool::load::{self, LoadError};

mod kin;
#[cfg(any(unix, windows))]
mod leftovers;
#[cfg(not(any(unix, windows)))]
mod other;
#[cfg(any(unix, windows))]
mod reply;
mod section;
#[cfg(any(unix, windows))]
mod serve;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use kin::{Binder, Kin};
#[cfg(not(any(unix, windows)))]
pub use other::{Channel, Handed, keep};
pub use section::{Example, Section};
#[cfg(any(unix, windows))]
pub use serve::Handed;
#[cfg(unix)]
pub use unix::{Channel, keep};
#[cfg(windows)]
pub use windows::{Channel, keep};

/// 这条命令是谁，脚本调工具的通道认人时照它：Unix 上是命令的进程组号。
#[cfg(unix)]
pub type Member = u32;

/// 同上：Windows 上是命令的作业对象。
#[cfg(windows)]
pub type Member = miyu_sandbox::lifeline::Job;

/// 同上：别的系统没有通道。
#[cfg(not(any(unix, windows)))]
pub type Member = ();

/// 运行日志的来处：脚本调工具的通道（施工 XG-1 下以前写 `miyu::shell`，挪出来以后 `shell`、扩展共用）。
pub(crate) const TARGET: &str = "miyu::call";

/// 资源目录 `resources` 里放这几句的目录：`core/call/`。
fn dir(resources: &Path) -> std::path::PathBuf {
    resources.join("core")
}

/// 读 `core/call/<name>.txt`，拿 `fields` 里的每个字段试换一次。
fn text(resources: &Path, name: &str, fields: &[&str]) -> Result<Template, LoadError> {
    load::text_in(&dir(resources), "call", name, fields)
}

/// 读 `core/call/<name>.txt` 的原文，不当模板读（说明里的例子，里面有花括号）。
fn raw(resources: &Path, name: &str) -> Result<String, LoadError> {
    let file = dir(resources).join("call").join(format!("{name}.txt"));
    std::fs::read_to_string(&file).map_err(|error| LoadError {
        file,
        why: error.to_string(),
    })
}

/// 通道答回去给她看的几句：退出码 1 到 4 的说法，`--schema` 印的样子。英文，一句，能指个改法的跟一个（设计 39 第六节第 4 条）。
#[cfg_attr(
    not(any(unix, windows)),
    expect(dead_code, reason = "别的系统没有通道，这几句用不上")
)]
pub struct Texts {
    stranger: Template,
    version: Template,
    bad_request: Template,
    ended: Template,
    interrupted: Template,
    unknown: Template,
    guess: Template,
    not_allowed: Template,
    bad_args: Template,
    approval: Template,
    images: Template,
    schema: Template,
    result: Template,
    /// 会话里的 `miyu pkg` 用不了（施工 BS-1 中）。
    pkg_off: Template,
}

impl Texts {
    /// 照资源目录 `resources` 读。
    ///
    /// # Errors
    ///
    /// 哪一句读不出来、换不进字段：写明是哪个文件。
    pub fn load(resources: &Path) -> Result<Arc<Texts>, LoadError> {
        Ok(Arc::new(Texts {
            stranger: text(resources, "stranger", &[])?,
            version: text(resources, "version", &[])?,
            bad_request: text(resources, "bad-request", &[])?,
            ended: text(resources, "ended", &[])?,
            interrupted: text(resources, "interrupted", &[])?,
            unknown: text(resources, "unknown", &["name"])?,
            guess: text(resources, "guess", &["name"])?,
            not_allowed: text(resources, "not-allowed", &["name", "tools"])?,
            bad_args: text(resources, "bad-args", &["name"])?,
            approval: text(resources, "approval", &["name"])?,
            images: text(resources, "images", &[])?,
            schema: text(resources, "schema", &["name", "description", "parameters"])?,
            result: text(resources, "result", &["result"])?,
            pkg_off: text(resources, "pkg-off", &[])?,
        }))
    }
}
