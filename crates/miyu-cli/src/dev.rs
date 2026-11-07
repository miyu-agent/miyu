//! `miyu dev`（`docs/blueprint/cli/dev.md`，施工 MD-3）：给写包的人用的几样。现在只有 `driver-check <包编号>`：连上核心（没在跑就
//! 拉起），发 `driver.check`（核心照样本调扩展的 `driver.encode`、中途重启扩展、比字节，设计 66 第七节第 2 条），把每个驱动、每份
//! 样本过没过印在标准输出上。
//!
//! 都过了、只有跳过的退出码 0；有红的 1；被拒（没装、没驱动、没在跑、有会话正在用它）照核心的原话，1。

use std::io::{self, Write};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::diagnostic;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu dev` 的参数。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Args)]
pub struct Dev {
    /// 做哪一样。
    #[command(subcommand)]
    pub command: DevCommand,
}

/// `miyu dev` 的几样。
#[derive(Debug, Clone, Subcommand)]
pub enum DevCommand {
    /// 查一个包提供的模型驱动编码稳不稳。
    DriverCheck {
        /// 包的编号。
        package: String,
    },
}

/// 跑一次 `miyu dev`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn dev(args: Dev, start: impl FnOnce() -> Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    ExitCode::from(runtime.block_on(run(args, start)))
}

/// 在运行时里：找数据根、连上核心、查。
async fn run(args: Dev, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return diagnostic::fail(error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return diagnostic::fail(error.to_string());
    }
    let (connection, token) = match connect_or_start(&root, start).await {
        Ok(connected) => connected,
        Err(error) => return diagnostic::fail(error.to_string()),
    };
    dev_on(
        connection,
        &token,
        &args,
        language,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做 `args`：握手、发、印，交回退出码。测试照它在进程里走一遍。
pub async fn dev_on(
    connection: Connection,
    token: &str,
    args: &Dev,
    language: Language,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "dev");
    let language = match link::hello(&mut rpc, token, &language, false, err).await {
        Ok(hello) => link::spoken(&hello, language),
        Err(code) => return code,
    };
    let DevCommand::DriverCheck { package } = &args.command;
    let params = json!({"package": package});
    match link::request(&mut rpc, "driver.check", params, &language, err).await {
        Ok(result) => {
            let (text, passed) = report(&result, language);
            say(out, &text);
            match passed {
                true => exit::OK,
                false => exit::ERROR,
            }
        }
        Err(code) => code,
    }
}

/// `driver.check` 的回应排成几行：一个驱动一行（驱动、缓存类型、结果），下面一份样本一行；没红的交回真。原因是核心的英文原话。
pub(crate) fn report(result: &Value, language: Language) -> (String, bool) {
    let mut lines = Vec::new();
    let mut passed = true;
    for driver in result["drivers"].as_array().into_iter().flatten() {
        let text = |key: &str| driver[key].as_str().unwrap_or_default();
        let outcome = text("result");
        passed &= outcome != "failed";
        let mut head = language.driver_checked(text("driver"), text("cache"), outcome);
        if !text("why").is_empty() {
            head = format!("{head}: {}", text("why"));
        }
        lines.push(head);
        for sample in driver["samples"].as_array().into_iter().flatten() {
            let name = sample["sample"].as_str().unwrap_or_default();
            let ok = sample["ok"].as_bool().unwrap_or_default();
            let mut line = format!("  {}", language.sample_checked(name, ok));
            if let Some(why) = sample["why"].as_str() {
                line = format!("{line}: {why}");
            }
            lines.push(line);
        }
    }
    (lines.join("\n"), passed)
}

#[cfg(test)]
mod tests;
