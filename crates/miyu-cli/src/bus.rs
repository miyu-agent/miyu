//! `miyu bus list [<名字>]`（施工 BUS-3，设计 `67-总线.md` 第四节「命令行」，照 `busctl list`）：印总线上谁提供了什么、可不可用。
//! 查「为什么终端里没有麦克风」「长消息为什么没转图」用。连上核心（没在跑就拉起来），发 `bus.list`，一行一个：名字、种类、包、
//! 可不可用，名字照最长的对齐；一个都没有的印一句。`--format json` 原样印回应。

use std::io::{self, Write};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::diagnostic;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu bus` 的参数。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Args)]
pub struct Bus {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<BusCommand>,
    /// 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// `miyu bus` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum BusCommand {
    /// 列出总线上的名字；写了名字的只列这一个。
    List {
        /// 只看哪一个名字。
        name: Option<String>,
    },
}

/// 跑一次 `miyu bus`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn bus(args: Bus, start: impl FnOnce() -> Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    ExitCode::from(runtime.block_on(run(args, start)))
}

async fn run(args: Bus, start: impl FnOnce() -> Command) -> u8 {
    let root = match DataRoot::locate(&Env::current()) {
        Ok(root) => root,
        Err(error) => return diagnostic::fail(error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return diagnostic::fail(error.to_string());
    }
    let connected = connect_or_start(&root, start)
        .await
        .map_err(|error| error.to_string());
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return diagnostic::fail(&reason),
    };
    let language = language::current();
    bus_on(
        connection,
        &token,
        (&args, language),
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、发 `bus.list`、印，交回退出码。测试照它在进程里走一遍。
pub async fn bus_on(
    connection: Connection,
    token: &str,
    (args, language): (&Bus, Language),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "bus");
    if let Err(code) = link::hello(&mut rpc, token, &language, false, err).await {
        return code;
    }
    let name = match &args.command {
        Some(BusCommand::List { name }) => name.clone(),
        None => None,
    };
    let params = name.map_or_else(|| json!({}), |name| json!({"name": name}));
    let listed = match link::answer(&mut rpc, "bus.list", params, &language, err).await {
        Ok(Ok(listed)) => listed,
        Ok(Err(_)) => return exit::ERROR,
        Err(code) => return code,
    };
    match args.format {
        Format::Json => say(out, &listed.to_string()),
        Format::Text => {
            for line in lines(&listed, language) {
                say(out, &line);
            }
        }
    }
    exit::OK
}

/// 一行一个：名字、种类、包、可不可用，名字和包照最长的对齐；一个都没有的是一句。
pub(crate) fn lines(listed: &Value, language: Language) -> Vec<String> {
    let names: Vec<&Value> = listed["names"].as_array().into_iter().flatten().collect();
    if names.is_empty() {
        return vec![language.bus_empty().to_string()];
    }
    let text = |one: &Value, key: &str| one[key].as_str().unwrap_or_default().to_string();
    let name_width = names
        .iter()
        .map(|one| text(one, "name").chars().count())
        .max()
        .unwrap_or(0);
    let package_width = names
        .iter()
        .map(|one| text(one, "package").chars().count())
        .max()
        .unwrap_or(0);
    names
        .iter()
        .map(|one| {
            let kind = text(one, "kind");
            let kind = language.bus_kind(&kind);
            let ready = language.bus_ready(one["ready"] == true);
            format!(
                "{:name_width$}  {kind}  {:package_width$}  {ready}",
                text(one, "name"),
                text(one, "package"),
            )
            .trim_end()
            .to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests;
