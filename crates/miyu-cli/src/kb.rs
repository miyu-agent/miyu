//! `miyu kb`（`docs/blueprint/cli/kb.md`，施工 K-7 下）：在 shell 里管知识库：列、搜、读、建、改、加、删、挂、重建。连上核心
//! （照 `miyu memory`：没在跑就拉起来），照子命令发 `kb.*`（`protocol.md`），照回应印。加文件照附件上传：每个文件先 `blob.put`
//! 本机路径，再 `kb.add`（2026-10-13 项目主人定）。
//!
//! 做成了退出码 0；核心拒的照核心的原话印在标准错误上，退出码 1；参数写错的 2，由 clap 管。

mod listing;
mod shown;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::knowledge::Knowledge;
use miyu_store::layers::valid;
use miyu_store::root::DataRoot;

use crate::ask::{Format, exit};
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu kb` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。选项写在子命令前后都行。
#[derive(Debug, Clone, Args)]
pub struct Kb {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<KbCommand>,
    /// `search`：只搜这一个库。
    #[arg(long = "kb", global = true, value_name = "KB")]
    pub only: Option<String>,
    /// `read`：第几页，从 1 起。
    #[arg(long, global = true, value_name = "PAGE")]
    pub page: Option<u64>,
    /// `new`、`edit`、`mount`：给人看的名字。
    #[arg(long, global = true, value_name = "NAME")]
    pub name: Option<String>,
    /// `new`、`edit`、`mount`：一句话说这个库是什么。
    #[arg(long, global = true, value_name = "DESCRIPTION")]
    pub description: Option<String>,
    /// `new`、`edit`、`mount`：开自动想起（施工 K-6 补，`knowledge.md` 第九条）。
    #[arg(long, global = true, conflicts_with = "no_auto")]
    pub auto: bool,
    /// `edit`：关自动想起（施工 K-6 补）；`new`、`mount` 本来就关着，写了也一样。
    #[arg(long = "no-auto", global = true)]
    pub no_auto: bool,
    /// `add`：放进库里的这个目录。
    #[arg(long, global = true, value_name = "DIR")]
    pub to: Option<String>,
    /// `add`：换掉已经有的。
    #[arg(long, global = true)]
    pub replace: bool,
    /// `list`、`search` 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// `miyu kb` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum KbCommand {
    /// 列出知识库。
    List,
    /// 搜：几个词用一个空格连起来。
    Search {
        /// 搜什么。
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
    },
    /// 读一篇的一页。
    Read {
        /// 哪个库。
        kb: String,
        /// 库里的哪一篇（相对路径）。
        path: String,
    },
    /// 建一个库。
    New {
        /// 编号。
        kb: String,
    },
    /// 改名字、说明。
    Edit {
        /// 哪个库。
        kb: String,
    },
    /// 把文件加进库。
    Add {
        /// 哪个库。
        kb: String,
        /// 加哪些文件。
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
    },
    /// 删一篇，挪进回收站。
    Remove {
        /// 哪个库。
        kb: String,
        /// 库里的哪一篇。
        path: String,
    },
    /// 把现成的目录挂成一个库。
    Mount {
        /// 编号。
        kb: String,
        /// 哪个目录。
        folder: PathBuf,
    },
    /// 删整个库，挪进回收站；挂进来的只拿掉登记。
    Delete {
        /// 哪个库。
        kb: String,
    },
    /// 重建索引。
    Reindex {
        /// 哪个库。
        kb: String,
    },
}

/// 这一次做什么，照哪种语言说，相对路径照哪个目录接。
#[derive(Debug, Clone)]
pub struct KbPlan {
    /// 读好的参数。
    pub args: Kb,
    /// 界面语言。
    pub language: Language,
    /// 当前目录：`add` 的文件、`mount` 的目录写的是相对路径的照它接成绝对的（核心只认绝对的）。
    pub cwd: PathBuf,
}

/// 跑一次 `miyu kb`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn kb(args: Kb, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、照子命令做。
async fn run(args: Kb, start: impl FnOnce() -> Command) -> u8 {
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(&error.to_string()),
    };
    let connected = connect_or_start(&root, start)
        .await
        .map_err(|error| error.to_string());
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return failed(&reason),
    };
    let plan = KbPlan {
        args,
        language: language::current(),
        cwd,
    };
    kb_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、照子命令发、印，交回退出码；印的写在 `out`，出错的写在 `err`。测试照它在进程里走一遍。
pub async fn kb_on(
    connection: Connection,
    token: &str,
    plan: &KbPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "kb");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    match act(&mut rpc, plan, out, err).await {
        Ok(()) => exit::OK,
        Err(code) => code,
    }
}

/// 照子命令发 `kb.*`，照回应印。
async fn act(
    rpc: &mut Rpc,
    plan: &KbPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let (args, language) = (&plan.args, &plan.language);
    // 库的编号写错的这里就说清写法（施工 K-7 修：核心只回「参数错误。」）。
    if let Some(id) = ids(args).into_iter().find(|id| !valid(id)) {
        say(err, &language.kb_bad_id(id));
        return Err(exit::MISUSE);
    }
    let auto = match (args.auto, args.no_auto) {
        (true, _) => Some(true),
        (_, true) => Some(false),
        _ => None,
    };
    let named = || json!({"name": args.name, "description": args.description, "auto": auto});
    let said = match args.command.clone().unwrap_or(KbCommand::List) {
        KbCommand::List => {
            let result = ask(rpc, "kb.list", json!({}), language, err).await?;
            let names = listing::package_names(rpc, &result, language, err).await?;
            return listing::list(&result, &names, plan, out);
        }
        KbCommand::Search { words } => {
            let params = json!({"query": words.join(" "), "kb": args.only});
            let result = ask(rpc, "kb.search", params, language, err).await?;
            return listing::search(&result, plan, out);
        }
        KbCommand::Read { kb, path } => return read(rpc, plan, &kb, &path, out, err).await,
        KbCommand::New { kb } => {
            ask(
                rpc,
                "kb.create",
                with(json!({"kb": kb}), named()),
                language,
                err,
            )
            .await?;
            language.kb_created(&kb)
        }
        KbCommand::Edit { kb } => {
            if args.name.is_none() && args.description.is_none() && auto.is_none() {
                say(err, language.kb_edit_nothing());
                return Err(exit::ERROR);
            }
            ask(
                rpc,
                "kb.update",
                with(json!({"kb": kb}), named()),
                language,
                err,
            )
            .await?;
            language.kb_updated(&kb)
        }
        KbCommand::Add { kb, files } => return add(rpc, plan, &kb, &files, out, err).await,
        KbCommand::Remove { kb, path } => {
            let params = json!({"kb": kb, "path": path});
            let result = ask(rpc, "kb.remove", params, language, err).await?;
            language.kb_removed(result["trash"].as_str().unwrap_or_default())
        }
        KbCommand::Mount { kb, folder } => {
            let folder = absolute(&plan.cwd, &folder);
            let params = with(json!({"kb": kb, "folder": folder}), named());
            let result = ask(rpc, "kb.mount", params, language, err).await?;
            language.kb_mounted(&kb, result["documents"].as_u64().unwrap_or(0))
        }
        KbCommand::Delete { kb } => {
            let result = ask(rpc, "kb.delete", json!({"kb": kb}), language, err).await?;
            language.kb_deleted(&kb, result["trash"].as_str().unwrap_or_default())
        }
        KbCommand::Reindex { kb } => {
            let result = ask(rpc, "kb.reindex", json!({"kb": kb}), language, err).await?;
            language.kb_reindexed(&kb, result["documents"].as_u64().unwrap_or(0))
        }
    };
    say(out, &said);
    Ok(())
}

/// `add`：一个个文件先照本机路径传上去，再加进库；碰到没成的那一个就停（前面加进去的不退），照原话说。
async fn add(
    rpc: &mut Rpc,
    plan: &KbPlan,
    kb: &str,
    files: &[PathBuf],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let (args, language) = (&plan.args, &plan.language);
    // 不认的格式在传以前就说清，一个都不传（施工 K-7 修：核心只回「参数错误。」，还白传一份）。
    for file in files {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        if !Knowledge::known(&name) {
            say(err, &language.kb_not_document(&name));
            return Err(exit::ERROR);
        }
    }
    for file in files {
        let local = absolute(&plan.cwd, file);
        let put = ask(rpc, "blob.put", json!({"path": local}), language, err).await?;
        let name = put["name"].as_str().unwrap_or_default();
        let path = match args.to.as_deref().map(|to| to.trim_matches('/')) {
            Some(to) if !to.is_empty() => format!("{to}/{name}"),
            _ => name.to_string(),
        };
        let params = json!({"kb": kb, "path": path, "blob": put["blob"], "replace": args.replace});
        ask(rpc, "kb.add", params, language, err).await?;
        say(out, &language.kb_added(kb, &path));
    }
    Ok(())
}

/// `read`：读一页印出来。写了 `--page`、核心说参数不对的是页码出界：再读一次第一页看一共几页，说清（施工 K-7 修）；别的错照
/// 原话说。
async fn read(
    rpc: &mut Rpc,
    plan: &KbPlan,
    kb: &str,
    path: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let (args, language) = (&plan.args, &plan.language);
    let params = json!({"kb": kb, "path": path, "page": args.page});
    match link::answer(rpc, "kb.read", without_nulls(params), language, err).await? {
        Ok(result) => {
            say(out, &shown::page(&result, language));
            Ok(())
        }
        Err(error) => {
            if let (Some(page), Some("bad_params")) = (args.page, error["data"]["reason"].as_str())
            {
                let first = ask(
                    rpc,
                    "kb.read",
                    json!({"kb": kb, "path": path}),
                    language,
                    err,
                )
                .await?;
                say(
                    err,
                    &language.kb_no_page(page, first["pages"].as_u64().unwrap_or(1)),
                );
            } else {
                say(err, &refusal(&error, language));
            }
            Err(exit::ERROR)
        }
    }
}

/// 这一次说到的知识库的编号（子命令的、`--kb` 的）。
fn ids(args: &Kb) -> Vec<&str> {
    let command = match &args.command {
        Some(
            KbCommand::Read { kb, .. }
            | KbCommand::New { kb }
            | KbCommand::Edit { kb }
            | KbCommand::Add { kb, .. }
            | KbCommand::Remove { kb, .. }
            | KbCommand::Mount { kb, .. }
            | KbCommand::Delete { kb }
            | KbCommand::Reindex { kb },
        ) => Some(kb.as_str()),
        Some(KbCommand::List | KbCommand::Search { .. }) | None => None,
    };
    command.into_iter().chain(args.only.as_deref()).collect()
}

/// 发一条，拒了的照原话印在 `err` 上：挂不上的目录、挪不进回收站的后面接上核心给的为什么。
async fn ask(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    language: &Language,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    link::request_saying(rpc, method, without_nulls(params), language, err, |error| {
        refusal(error, language)
    })
    .await
}

/// 核心拒了说的那一句：原话；挂不上的目录、挪不进回收站的接上它给的为什么。
fn refusal(error: &Value, language: &Language) -> String {
    let said = error["message"].as_str().unwrap_or_default();
    match error["data"]["message"].as_str() {
        Some(why) => language.refused_because(said, why),
        None => language.refused(said),
    }
}

/// `params` 里加上 `extra` 写了的几格。
fn with(mut params: Value, extra: Value) -> Value {
    if let (Value::Object(params), Value::Object(extra)) = (&mut params, extra) {
        params.extend(extra);
    }
    params
}

/// `null` 的几格不写（没给的选项）。
fn without_nulls(params: Value) -> Value {
    match params {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, value)| !value.is_null())
                .collect(),
        ),
        other => other,
    }
}

/// 相对路径照 `cwd` 接成绝对的；`~` 开头的、绝对的照原样（核心认 `~/`）。
fn absolute(cwd: &Path, path: &Path) -> String {
    let text = path.to_string_lossy();
    match path.is_absolute() || text.starts_with('~') {
        true => text.into_owned(),
        false => cwd.join(path).to_string_lossy().into_owned(),
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;
