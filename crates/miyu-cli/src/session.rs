//! `miyu session`（施工 SE-1 中上、中下下，`docs/blueprint/cli/session.md`，设计 `58-会话导出和导入.md`）：会话的原始包、
//! Markdown。
//!
//! - `miyu session export [<会话>] [-o <文件>]`：交 `session.export {format: "bundle"}`，核心写到那个文件；不写会话的是上一次
//!   `miyu ask` 开的那个（同 `miyu rename`），不写 `-o` 的是当前目录下的 `miyu-session-<编号前 8 位>.zip`。写好了在标准输出
//!   印一行写到哪（绝对路径），好接脚本。
//! - `--md [--for agent|chat] [--last N]`（施工 SE-1 中下下，调研第五节第 5 条）：交 `session.export {format: "markdown"}`，
//!   `--for` 是 `for`（不写的是 `read`），`--last N` 是 `range: {"turns": N}`；不写 `-o` 的是 `miyu-session-<编号前 8 位>.md`。
//!   `-o -` 走 `to: "inline"`，Markdown 原样打到标准输出（好接管道、`wl-copy` 这些，不做剪贴板）；超了上限的照核心的话报错，
//!   提示改成 `-o <文件>`。
//! - `miyu session import <文件> [--archive]`：交 `session.import {path}`；新导进来的会话一行一个印在标准输出，早就在的不印。
//! - 相对路径照当前目录接成绝对路径：核心不认相对的。被拒的照核心的原话印在标准错误上，退出码 1；核心没在跑、又没设 key 的 5。

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::json;

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::diagnostic;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;

/// `miyu session` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Session {
    /// 做什么。
    #[command(subcommand)]
    pub command: SessionCommand,
}

/// `miyu session` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    /// 导出成原始包，或者 Markdown。
    Export {
        /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
        session: Option<String>,
        /// 写到哪个文件；不写的是当前目录下的 `miyu-session-<编号前 8 位>.zip`（Markdown 是 `.md`）。Markdown 写 `-` 的打到
        /// 标准输出。
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
        /// 导成 Markdown。
        #[arg(long)]
        md: bool,
        /// Markdown 给谁用：`read`（不写的也是）、`agent`、`chat`。
        #[arg(long = "for", requires = "md", value_parser = ["read", "agent", "chat"])]
        purpose: Option<String>,
        /// Markdown 只要最近几轮。
        #[arg(long, requires = "md", value_parser = clap::value_parser!(u32).range(1..))]
        last: Option<u32>,
    },
    /// 导入原始包。
    Import {
        /// 包文件。
        file: PathBuf,
        /// 导成只读存档。
        #[arg(long)]
        archive: bool,
    },
}

/// 跑一次 `miyu session`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn session(args: Session, start: impl FnOnce() -> Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return ExitCode::from(diagnostic::fail(error.to_string())),
    };
    ExitCode::from(runtime.block_on(run(args, start)))
}

/// 在运行时里：找数据根、连上核心、照子命令办。
async fn run(args: Session, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let here = match std::env::current_dir() {
        Ok(here) => here,
        Err(error) => return diagnostic::fail(error.to_string()),
    };
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
    let mut out = io::stdout();
    let mut err = io::stderr();
    session_on(
        connection,
        &token,
        &args,
        (&here, language),
        (&mut out, &mut err),
    )
    .await
}

/// 在一条连上了的连接上办一次：握手、照子命令发请求，交回退出码；做成的写在 `out` 上，出错的写在 `err` 上。相对路径照 `here`
/// 接。测试照它在进程里走一遍。
pub async fn session_on(
    connection: Connection,
    token: &str,
    args: &Session,
    (here, language): (&Path, Language),
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> u8 {
    let mut rpc = Rpc::new(connection, "session");
    if let Err(code) = link::hello(&mut rpc, token, &language, false, err).await {
        return code;
    }
    match &args.command {
        SessionCommand::Export {
            session,
            output,
            md,
            purpose,
            last,
        } => {
            let session = match session {
                Some(session) => session.clone(),
                None => match link::latest_oneshot(&mut rpc, &language, err).await {
                    Ok(session) => session,
                    Err(code) => return code,
                },
            };
            let wanted = Wanted {
                md: *md,
                purpose: purpose.as_deref(),
                last: *last,
            };
            export(
                &mut rpc,
                &session,
                output.as_deref(),
                wanted,
                (here, language),
                (out, err),
            )
            .await
        }
        SessionCommand::Import { file, archive } => {
            let params = json!({"path": here.join(file), "archive": archive});
            match link::request(&mut rpc, "session.import", params, &language, err).await {
                Ok(result) => {
                    let added: Vec<&str> = result["sessions"]
                        .as_array()
                        .map(|list| list.iter().filter_map(|id| id.as_str()).collect())
                        .unwrap_or_default();
                    said(out, &added.join("\n"))
                }
                Err(code) => code,
            }
        }
    }
}

/// `export` 要的格式：`--md`、`--for`、`--last`。
struct Wanted<'a> {
    /// 导成 Markdown。
    md: bool,
    /// Markdown 给谁用。
    purpose: Option<&'a str>,
    /// Markdown 只要最近几轮。
    last: Option<u32>,
}

/// `miyu session export`：照格式发 `session.export`。写文件的印写到哪，`-o -` 的把 Markdown 原样打到 `out` 上。
async fn export(
    rpc: &mut Rpc,
    session: &str,
    output: Option<&Path>,
    wanted: Wanted<'_>,
    (here, language): (&Path, Language),
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> u8 {
    let extension = if wanted.md { "md" } else { "zip" };
    let inline = wanted.md && output == Some(Path::new("-"));
    let path = match output {
        Some(output) => here.join(output),
        None => here.join(format!(
            "miyu-session-{}.{extension}",
            session.chars().take(8).collect::<String>()
        )),
    };
    let mut params = json!({"session": session, "format": "bundle", "to": {"path": path}});
    if wanted.md {
        params["format"] = json!("markdown");
        params["for"] = json!(wanted.purpose.unwrap_or("read"));
        if let Some(last) = wanted.last {
            params["range"] = json!({"turns": last});
        }
        if inline {
            params["to"] = json!("inline");
        }
    }
    let reply = link::request_saying(rpc, "session.export", params, &language, err, |error| {
        let said = language.refused(error["message"].as_str().unwrap_or_default());
        match (inline, error["data"]["reason"].as_str()) {
            (true, Some("export_too_big")) => said.hint(language.export_to_file()),
            _ => said,
        }
    })
    .await;
    match reply {
        Ok(result) if inline => {
            let text = result["text"].as_str().unwrap_or_default();
            match out.write_all(text.as_bytes()) {
                Ok(()) => exit::OK,
                Err(error) => diagnostic::fail(error.to_string()),
            }
        }
        Ok(_) => said(out, &path.display().to_string()),
        Err(code) => code,
    }
}

/// 在 `out` 上印一行（空的不印），交回退出码。
fn said(out: &mut dyn Write, line: &str) -> u8 {
    if line.is_empty() {
        return exit::OK;
    }
    match writeln!(out, "{line}") {
        Ok(()) => exit::OK,
        Err(error) => diagnostic::fail(error.to_string()),
    }
}
