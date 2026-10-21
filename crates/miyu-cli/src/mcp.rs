//! `miyu mcp login <服务>`（`docs/blueprint/cli/mcp.md`，施工 X-42 上）：网址的 MCP 服务要浏览器登录的，连上核心（没在跑的
//! 拉起）发 `mcp.login`，印出授权页的地址、试着打开浏览器，等服务连上（个人设置里的服务照 `mcp.list` 看，最多 10 分钟）。只是协议
//! 的客户端：令牌由核心换、存，这里一个字也不经手；印出来的只有授权页的地址。
//!
//! 退出码：0 连上了（包里带的服务印了地址就算）；1 核心拒绝了、等到时还没连上、连不上核心。

use std::io::{self, Write};
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::connect_or_start;
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// 等服务连上最多多久：同核心等回调的时限。
const WAIT: Duration = Duration::from_secs(600);

/// 多久看一次连上了没有。
const EVERY: Duration = Duration::from_secs(1);

/// `miyu mcp` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Mcp {
    /// 子命令。
    #[command(subcommand)]
    pub command: McpCommand,
}

/// `miyu mcp` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum McpCommand {
    /// 登录一个网址的服务。
    Login {
        /// 服务名。
        #[arg(value_name = "SERVICE")]
        service: String,
    },
}

/// 跑一次 `miyu mcp`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn mcp(args: Mcp, start: impl FnOnce() -> Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit::ERROR);
        }
    };
    ExitCode::from(runtime.block_on(run(args, start)))
}

/// 在运行时里：找数据根、连上核心、登录。
async fn run(args: Mcp, start: impl FnOnce() -> Command) -> u8 {
    let McpCommand::Login { service } = args.command;
    let language = language::current();
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    let (connection, token) = match connect_or_start(&root, start).await {
        Ok(connected) => connected,
        Err(error) => return failed(&error.to_string()),
    };
    let mut rpc = Rpc::new(connection, "mcp");
    let err = &mut io::stderr();
    let language = match link::hello(&mut rpc, &token, &language, false, err).await {
        Ok(hello) => link::spoken(&hello, language),
        Err(code) => return code,
    };
    let answer = match link::request(
        &mut rpc,
        "mcp.login",
        json!({"service": service}),
        &language,
        err,
    )
    .await
    {
        Ok(answer) => answer,
        Err(code) => return code,
    };
    let url = answer["url"].as_str().unwrap_or_default();
    let out = &mut io::stdout();
    say(out, &language.open_to_log_in(&service));
    say(out, url);
    drop(out.flush());
    if !opened(url) {
        say(err, language.browser_not_opened());
    }
    waited(&mut rpc, &service, &language, out, err).await
}

/// 等服务连上：个人设置里的照 `mcp.list` 看；不在里面的（包里带的）印一句就算。
async fn waited(
    rpc: &mut Rpc,
    service: &str,
    language: &Language,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let listed = match link::request(rpc, "mcp.list", json!({}), language, err).await {
            Ok(listed) => listed,
            Err(code) => return code,
        };
        match state(&listed, service) {
            None => {
                say(out, language.connects_by_itself());
                return exit::OK;
            }
            Some("running") => {
                say(out, &language.logged_in(service));
                return exit::OK;
            }
            Some(_) if tokio::time::Instant::now() >= deadline => {
                say(err, &language.login_timed_out(service));
                return exit::ERROR;
            }
            Some(_) => tokio::time::sleep(EVERY).await,
        }
    }
}

/// `mcp.list` 里服务 `service` 这时的状态；不在里面的没有。
fn state<'a>(listed: &'a Value, service: &str) -> Option<&'a str> {
    listed["servers"]
        .as_array()?
        .iter()
        .find(|one| one["name"] == service)
        .map(|one| one["state"].as_str().unwrap_or_default())
}

/// 用系统的办法打开 `url`：打开得了的交回 `true`。不等浏览器退出。
fn opened(url: &str) -> bool {
    let (program, args) = opener(url);
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// 打开网址的程序和参数：Linux 这类 `xdg-open`，macOS `open`，Windows 经 `rundll32` 交给默认浏览器（`cmd /c start` 会把网址里的
/// `&` 当成分隔符）。
fn opener(url: &str) -> (&'static str, Vec<String>) {
    if cfg!(target_os = "windows") {
        (
            "rundll32.exe",
            vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()],
        )
    } else if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else {
        ("xdg-open", vec![url.to_string()])
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;
