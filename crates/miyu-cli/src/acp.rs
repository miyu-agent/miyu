//! `miyu acp`（设计 `73-ACP.md` 第五节，施工 ACP-6，`internal/blueprint/cli/acp.md`）：Miyu 当 ACP（Agent Client Protocol）的
//! agent。编辑器（Zed、JetBrains 这类）把它当子进程拉起，在标准输入、标准输出上说 JSON-RPC 2.0，一行一条；它连本机的核心
//! （照 `miyu ask` 一样连、没在跑就拉起来），把 ACP 翻成核心的会话协议。不另起模型、不另存会话：编辑器里的会话就是 Miyu 的
//! 会话，终端、网页里也看得到、接得上。
//!
//! 标准输出上只有协议的行；连不上核心这类开局的错误写在标准错误上，编辑器收进它的日志。之后核心断了不退出，再连上
//! （施工 ACP-9，`relink.rs`）。

mod asking;
mod choosing;
mod editor;
mod forms;
mod options;
mod prompt;
mod relink;
mod server;
mod session;
mod setup;
mod status;
mod tool;
mod turns;
mod view;

pub use relink::{Linked, Reconnect};
pub use server::serve;

use std::process::{Command, ExitCode};

use clap::Args;

use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::diagnostic;
use crate::language;
use crate::startup::connect_or_start;

/// `miyu acp` 的参数：没有。给人看的说明在帮助页里（`help/zh/acp.txt`）。
#[derive(Debug, Clone, Args)]
pub struct Acp {}

/// 跑 `miyu acp`，交回退出码：编辑器关了管道是 0，头一次连不上核心是 1。`start` 给出拉起核心的命令：头一次、断了以后
/// 再连都用它。
pub fn acp(_args: Acp, start: impl Fn() -> Command + Send + Sync + 'static) -> ExitCode {
    let language = language::current();
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    ExitCode::from(runtime.block_on(async move {
        let env = Env::current();
        let root = match DataRoot::locate(&env) {
            Ok(root) => root,
            Err(error) => return diagnostic::fail(crate::startup::spoken(&error)),
        };
        if let Err(error) = root.prepare() {
            return diagnostic::fail(crate::startup::spoken(&error));
        }
        let (connection, token) = match connect_or_start(&root, &start).await {
            Ok(connected) => connected,
            Err(error) => return diagnostic::fail(crate::startup::spoken(&error)),
        };
        let reconnect = Box::new(relink::Local { root, start });
        let input = tokio::io::BufReader::new(tokio::io::stdin());
        serve(
            connection,
            &token,
            reconnect,
            input,
            tokio::io::stdout(),
            language,
        )
        .await
    }))
}
