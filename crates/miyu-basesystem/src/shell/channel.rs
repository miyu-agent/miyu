//! 脚本调工具的通道（施工 M-3，设计 `39-脚本调工具.md` 第五节，`docs/blueprint/tools/shell.md`「脚本调工具」）：直接调的
//! `shell` 拿到了端口（[`miyu_tool::Call::nested`]）的，起命令前开一个只给这条命令的端点，命令里的 `miyu call` 连上来调工具。
//! 端点、协议、认人在 `miyu-call`（施工 XG-1 下挪过去，扩展声明了 `runs_commands` 的工具也用它）；这里只管 `shell` 这一头：照
//! 这条命令的进程组、作业对象认人（[`miyu_call::Kin::command`]），环境变量、沙盒的规格照它加上。

use std::io;
use std::sync::Arc;

use miyu_call::{Channel, Kin};
use miyu_tool::{Call, Stop};

use super::{Shell, env, workdir};
use crate::blocking::blocking;

impl Shell {
    /// 造命令：拿到了端口的开通道（施工 M-3），环境变量、沙盒的规格照它加上；开不成的记一行，命令照旧跑。
    ///
    /// 带沙盒的命令要认指纹的（[`miyu_sandbox::checks_files`]，施工 V-50）在阻塞线程里造，不占着跑异步任务的线程；造好的
    /// 命令交回来，起进程（`fork`）照旧在调的一方。
    pub(super) async fn prepare(
        &self,
        call: &Call,
        script: &str,
    ) -> io::Result<(std::process::Command, Option<Channel>)> {
        let channel = call.nested.clone().and_then(|port| {
            Channel::open(port, Arc::clone(&self.call), Kin::command())
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
        let cwd = workdir(call);
        let command = match sandbox.as_ref().is_some_and(miyu_sandbox::checks_files) {
            true => {
                let (program, script) = (self.program.clone(), script.to_owned());
                blocking(Stop::default(), move |_| {
                    program.command(&script, &cwd, env, sandbox.as_ref())
                })
                .await?
            }
            false => self.program.command(script, &cwd, env, sandbox.as_ref())?,
        };
        Ok((command, channel))
    }
}
