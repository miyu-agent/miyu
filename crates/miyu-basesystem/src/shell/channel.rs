//! 脚本调工具的通道（施工 M-3，设计 `39-脚本调工具.md` 第五节，`docs/blueprint/tools/shell.md`「脚本调工具」）：直接调的
//! `shell` 拿到了端口（[`miyu_tool::Call::nested`]）的，起命令前开一个只给这条命令的端点，命令里的 `miyu call` 连上来调工具。
//!
//! - 端点：一个 Unix 套接字，放在临时目录下一个核心进程一个、只有本人进得去的目录里（`0700`），名字随机。告诉子进程的是
//!   `MIYU_CALL_SOCK`（路径）、`MIYU_CALL_TOKEN`（128 位随机的令牌），在 `env::passed` 之后加：所有语言起子进程都默认继承。
//!   `PATH` 最前面放一个只有 `miyu` 的目录，指向核心自己：脚本里调到的一定是同一个版本。
//! - 认人：令牌对得上，连上来的进程在这条命令的进程组里（对方的进程号经 `SO_PEERCRED` / `LOCAL_PEERPID` 拿，查它的组）。
//!   过不了的回退出码 2。令牌漏了也只有这条命令自己的进程树用得上；`setsid` 出去的从一开始就调不成。
//! - 一次连接一个请求、一个回答：4 字节大端长度加 UTF-8 JSON。请求帧超过 1 MiB 的断开；同时连着的超过 32 个，多的断开。
//! - 丢掉 [`Channel`] 就关：服务的任务掐掉、套接字删掉，令牌跟着作废，脚本里等着的 `miyu call` 读到结尾（退出码 3）。前台的
//!   命令结束时丢；放到后台的跟着进程交给任务表，任务结束时丢（[`keep`]）。
//!
//! Windows 上是命名管道、照作业对象认人（施工 M-3 下，`channel/windows.rs`）；协议、令牌、退出码两边一样（`channel/serve.rs`）。

use std::io;
use std::sync::Arc;

use miyu_kernel::template::Template;

use miyu_tool::Call;

use super::{Shell, env, workdir};
use crate::load::{self, LoadError};

#[cfg(not(any(unix, windows)))]
mod other;
#[cfg(any(unix, windows))]
mod reply;
#[cfg(any(unix, windows))]
mod serve;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(any(unix, windows)))]
pub(super) use other::{Channel, keep};
#[cfg(unix)]
pub(super) use unix::{Channel, keep};
#[cfg(windows)]
pub(super) use windows::{Channel, keep};

/// 给她看的几句（`software/basesystem/shell/call/`）：退出码 1 到 4 的说法，`--schema` 印的样子。英文，一句，能指个改法的
/// 跟一个（设计 39 第六节第 4 条）。
#[cfg_attr(
    not(any(unix, windows)),
    expect(dead_code, reason = "别的系统没有通道，这几句用不上")
)]
pub(super) struct Texts {
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
}

impl Texts {
    /// 照资源目录 `resources` 读。
    pub(super) fn load(resources: &std::path::Path) -> Result<Arc<Texts>, LoadError> {
        let text = |name: &str, fields: &[&str]| {
            load::text(resources, "shell", &format!("call/{name}"), fields)
        };
        Ok(Arc::new(Texts {
            stranger: text("stranger", &[])?,
            version: text("version", &[])?,
            bad_request: text("bad-request", &[])?,
            ended: text("ended", &[])?,
            interrupted: text("interrupted", &[])?,
            unknown: text("unknown", &["name"])?,
            guess: text("guess", &["name"])?,
            not_allowed: text("not-allowed", &["name", "tools"])?,
            bad_args: text("bad-args", &["name"])?,
            approval: text("approval", &["name"])?,
            images: text("images", &[])?,
            schema: text("schema", &["name", "description", "parameters"])?,
            result: text("result", &["result"])?,
        }))
    }
}

impl Shell {
    /// 造命令：拿到了端口的开通道（施工 M-3），环境变量、沙盒的规格照它加上；开不成的记一行，命令照旧跑。
    pub(super) fn prepare(
        &self,
        call: &Call,
        script: &str,
    ) -> io::Result<(std::process::Command, Option<Channel>)> {
        let channel = call.nested.clone().and_then(|port| {
            Channel::open(port, Arc::clone(&self.call))
                .inspect_err(|error| {
                    tracing::warn!(target: "miyu::shell", error = %error, "miyu call endpoint not opened");
                })
                .ok()
        });
        let mut env = env::passed(std::env::vars_os(), call.sandbox.is_some());
        env.extend(env::told(call.caller.as_ref()));
        let sandbox = call.sandbox.as_deref().map(|sandboxed| match &channel {
            Some(channel) => channel.sandboxed(sandboxed),
            None => sandboxed.clone(),
        });
        if let Some(channel) = &channel {
            channel.env(&mut env);
        }
        let command = self
            .program
            .command(script, &workdir(call), env, sandbox.as_ref())?;
        Ok((command, channel))
    }
}
