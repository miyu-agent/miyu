//! `miyu schedule`（`docs/blueprint/cli/schedule.md`，施工 J-3 下，设计 45 第三节第 3 条、第五节第 3 条）：在 shell 里管定时任务：
//! 列、建、删、停、续。连上核心（照 `miyu kb`：没在跑就拉起来），照子命令发 `schedule.*`（`docs/reference/schedules.md`），照回应印。
//! 建 `cron` 的交本机的时区（IANA 名），`--zone` 手写的照它；`--at` 不带偏移的照本机时区换算；时刻都换成核心认的写法再交。
//!
//! 做成了退出码 0；核心拒的照核心的原话印在标准错误上（参数错的照 `data.problem` 说清是哪一样），退出码 1；参数写错的 2，由
//! clap 管，时刻、间隔读不懂的也是 2。

mod shown;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{ArgGroup, Args, Subcommand};
use jiff::tz::TimeZone;
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::{Format, exit};
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

pub use shown::{interval, moment};

/// `miyu schedule` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Schedule {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<ScheduleCommand>,
    /// `list` 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// `miyu schedule` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum ScheduleCommand {
    /// 列出这个账号的定时任务。
    List {
        /// 只列跑在这个会话上的。
        #[arg(long, value_name = "SESSION")]
        session: Option<String>,
    },
    /// 建一个：参数多，装在盒子里，别的几个不跟着变大。
    Add(Box<Add>),
    /// 删掉一个。
    Rm {
        /// 编号。
        id: String,
    },
    /// 停一个。
    Pause {
        /// 编号。
        id: String,
    },
    /// 续一个。
    Resume {
        /// 编号。
        id: String,
    },
}

/// `add` 的参数：时间三选一，目标是已有的会话或者每次新开。
#[derive(Debug, Clone, Args)]
#[command(group(ArgGroup::new("when").required(true).args(["cron", "at", "every"])))]
pub struct Add {
    /// 交进会话的话：几个词用一个空格连起来。
    #[arg(required = true, num_args = 1..)]
    pub words: Vec<String>,
    /// 标准五段的 cron。
    #[arg(long, value_name = "CRON")]
    pub cron: Option<String>,
    /// `cron` 只跑一次（`recurring` 是 false）。
    #[arg(long, conflicts_with_all = ["at", "every"])]
    pub once: bool,
    /// `cron` 照哪个时区读（IANA 名）；不写的交本机的。
    #[arg(long, value_name = "ZONE", conflicts_with_all = ["at", "every"])]
    pub zone: Option<String>,
    /// 一次：带偏移的 RFC 3339，或者照本机时区读的 `2026-10-28 08:00`。
    #[arg(long, value_name = "WHEN")]
    pub at: Option<String>,
    /// 每隔一阵：秒数，或者 `90s`、`10m`、`2h`、`1d`。
    #[arg(long, value_name = "INTERVAL")]
    pub every: Option<String>,
    /// 跑在这个已有的会话上。
    #[arg(long, value_name = "SESSION", conflicts_with_all = ["persona", "no_persona", "preset", "cwd"])]
    pub session: Option<String>,
    /// 每次新开：用这个人格。
    #[arg(long, value_name = "PERSONA")]
    pub persona: Option<String>,
    /// 每次新开：无人格。
    #[arg(long = "no-persona", conflicts_with = "persona")]
    pub no_persona: bool,
    /// 每次新开：用这个预设。
    #[arg(long, value_name = "PRESET")]
    pub preset: Option<String>,
    /// 每次新开：在这个目录里开。
    #[arg(long, value_name = "DIR")]
    pub cwd: Option<PathBuf>,
}

/// 这一次做什么，照哪种语言说，相对路径照哪个目录接，时刻照哪个时区读、印。
#[derive(Debug, Clone)]
pub struct SchedulePlan {
    /// 读好的参数。
    pub args: Schedule,
    /// 界面语言。
    pub language: Language,
    /// 当前目录：`--cwd` 写的是相对路径的照它接成绝对的。
    pub cwd: PathBuf,
    /// 本机的时区：`--at` 不带偏移的照它读，印的时刻照它换。
    pub zone: TimeZone,
}

/// 跑一次 `miyu schedule`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn schedule(args: Schedule, start: impl FnOnce() -> Command) -> ExitCode {
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
async fn run(args: Schedule, start: impl FnOnce() -> Command) -> u8 {
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
    let plan = SchedulePlan {
        args,
        language: language::current(),
        cwd,
        zone: TimeZone::try_system().unwrap_or(TimeZone::UTC),
    };
    schedule_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、照子命令发、印，交回退出码；印的写在 `out`，出错的写在 `err`。测试照它在进程里走一遍。
pub async fn schedule_on(
    connection: Connection,
    token: &str,
    plan: &SchedulePlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "schedule");
    if let Err(code) = link::hello(&mut rpc, token, &plan.language, false, err).await {
        return code;
    }
    match act(&mut rpc, plan, out, err).await {
        Ok(()) => exit::OK,
        Err(code) => code,
    }
}

/// 照子命令发 `schedule.*`，照回应印。
async fn act(
    rpc: &mut Rpc,
    plan: &SchedulePlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let language = &plan.language;
    let command = plan
        .args
        .command
        .clone()
        .unwrap_or(ScheduleCommand::List { session: None });
    let said = match command {
        ScheduleCommand::List { session } => {
            let params = without_nulls(json!({"session": session}));
            let result = ask(rpc, "schedule.list", params, language, err).await?;
            let listed = result["schedules"].as_array().cloned().unwrap_or_default();
            if plan.args.format == Format::Json {
                say(out, &Value::Array(listed).to_string());
                return Ok(());
            }
            if listed.is_empty() {
                say(out, language.no_schedules());
                return Ok(());
            }
            for line in shown::lines(&listed, language, &plan.zone) {
                say(out, &line);
            }
            return Ok(());
        }
        ScheduleCommand::Add(add) => {
            let params = match added(&add, plan) {
                Ok(params) => params,
                Err(said) => {
                    say(err, &said);
                    return Err(exit::MISUSE);
                }
            };
            let result = ask(rpc, "schedule.add", params, language, err).await?;
            language.schedule_added(&shown::line(&result["schedule"], language, &plan.zone))
        }
        ScheduleCommand::Rm { id } => {
            ask(rpc, "schedule.cancel", json!({"id": id}), language, err).await?;
            language.schedule_removed(&id)
        }
        ScheduleCommand::Pause { id } => {
            let result = ask(rpc, "schedule.pause", json!({"id": id}), language, err).await?;
            language.schedule_paused(&shown::line(&result["schedule"], language, &plan.zone))
        }
        ScheduleCommand::Resume { id } => {
            let result = ask(rpc, "schedule.resume", json!({"id": id}), language, err).await?;
            language.schedule_resumed(&shown::line(&result["schedule"], language, &plan.zone))
        }
    };
    say(out, &said);
    Ok(())
}

/// `add` 交给核心的参数；时刻、间隔读不懂的交回说法。
fn added(add: &Add, plan: &SchedulePlan) -> Result<Value, String> {
    let language = &plan.language;
    let when = if let Some(cron) = &add.cron {
        // 不写 `--zone` 的交本机的（IANA 名）；本机的说不出名字的不交，照核心所在机器的。
        let zone = add
            .zone
            .clone()
            .or_else(|| plan.zone.iana_name().map(str::to_string));
        without_nulls(json!({"cron": cron, "recurring": !add.once, "zone": zone}))
    } else if let Some(at) = &add.at {
        let at = moment(at, &plan.zone).ok_or_else(|| language.schedule_bad_at(at))?;
        json!({"at": at})
    } else {
        let every = add.every.as_deref().unwrap_or_default();
        let seconds = interval(every).ok_or_else(|| language.schedule_bad_every(every))?;
        json!({"every": seconds})
    };
    let target = match &add.session {
        Some(session) => json!({"session": session}),
        None => {
            let mut new = without_nulls(json!({
                "preset": add.preset,
                "cwd": add.cwd.as_deref().map(|cwd| real(&plan.cwd, cwd)),
            }));
            // 写了 `--no-persona` 的交 `null`（无人格），都没写的不交（照核心的默认）。
            if let Some(persona) = &add.persona {
                new["persona"] = json!(persona);
            } else if add.no_persona {
                new["persona"] = Value::Null;
            }
            json!({"new": new})
        }
    };
    Ok(json!({"when": when, "prompt": add.words.join(" "), "target": target}))
}

/// 发一条，拒了的照原话印在 `err` 上：参数错的照 `data.problem` 说清是哪一样。
async fn ask(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    language: &Language,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    link::request_saying(rpc, method, params, language, err, |error| {
        let said = error["message"].as_str().unwrap_or_default();
        match error["data"]["problem"]
            .as_str()
            .and_then(|problem| language.schedule_problem(problem))
        {
            Some(why) => language.refused_because(said, why),
            None => language.refused(said),
        }
    })
    .await
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

/// `--cwd`：相对路径照 `cwd` 接成绝对的，在的换成真实的位置（链接解开）；`~` 开头的照原样（核心认 `~/`），不在的照接好的交，
/// 核心说不在（`cwd`）。
fn real(cwd: &Path, path: &Path) -> String {
    if path.to_string_lossy().starts_with('~') {
        return path.to_string_lossy().into_owned();
    }
    let joined = cwd.join(path);
    std::fs::canonicalize(&joined)
        .unwrap_or(joined)
        .to_string_lossy()
        .into_owned()
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;
