//! 怎么起一个服务（施工 X-3 上，`mcp.md`「怎么走」第二条第 2、5 条）：程序照命令第一项找，工作目录是状态目录里它的一格，
//! 环境是核心的加上清单写的，标准错误接到日志目录里它的一份。不进沙盒。网址的服务（施工 X-32 上）不起进程：计划里是网址和
//! 请求头，令牌换成 `Authorization`，连的时候认它出错写进同一份日志。

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use miyu_config::package::{EnvValue, McpServer, Reach};
use miyu_store::packages::BIN;

use super::TARGET;
use crate::Core;
use crate::extensions::stderr;

/// 程序正被别的进程开着写时，起最多试几次、每次隔多久：同扩展（施工 F-8 上再补）。
const BUSY_TRIES: u32 = 20;
const BUSY_WAIT: Duration = Duration::from_millis(50);

/// Windows 上不弹控制台窗口：同扩展。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 怎么起一个服务。
#[derive(Debug, Clone)]
pub(super) struct Plan {
    /// 服务：名字、时限、给哪种会话。
    pub(super) server: McpServer,
    /// 起进程，还是连网址。
    pub(super) how: How,
    /// 工作目录：`state/mcp/<服务名>/`，没有的建。
    pub(super) dir: PathBuf,
    /// 标准错误接到哪个文件。
    pub(super) log: PathBuf,
    /// 缓存记着的时代（施工 X-3 下）：连的时候照它直接说；没有的先探。
    pub(super) known: Option<miyu_mcp::Era>,
}

/// 怎么到一个服务。
#[derive(Debug, Clone)]
pub(super) enum How {
    /// 起进程。
    Spawn {
        /// 程序：找到的位置，只有名字的照 `PATH` 找。
        program: PathBuf,
        /// 参数：命令第一项后面的。
        args: Vec<String>,
        /// 另给的环境变量，引用的已经取出来了。
        env: Vec<(String, String)>,
    },
    /// 连网址（施工 X-32 上）。
    Connect {
        /// 网址。
        url: String,
        /// 每个请求都带的头，令牌已经换成 `Authorization: Bearer …`、引用的已经取出来了。
        headers: Vec<(String, String)>,
        /// 登录来的（施工 X-42 下）：这张表自己没写令牌、存着登录的那一份；转接每个请求现场问令牌（`login::Login`），不在这里带。
        login: bool,
    },
}

/// 包 `package`（包目录 `dir`）带的服务 `server` 怎么起：引用的密钥、环境变量照这一刻的配置取，取不到的那一个不设、记一行
/// `WARN`。
pub(super) fn plan(core: &Core, package: &str, server: &McpServer, dir: &Path) -> Plan {
    let config = core.config_now().borrow().clone();
    let resolve = |pairs: &[(String, EnvValue)]| -> Vec<(String, String)> {
        pairs
            .iter()
            .filter_map(|(name, value)| {
                let resolved = resolved(&config, value);
                if resolved.is_none() {
                    tracing::warn!(target: TARGET, package, service = server.name.as_str(), variable = name.as_str(), "mcp env not set");
                }
                resolved.map(|value| (name.clone(), value))
            })
            .collect()
    };
    let how = match &server.reach {
        Reach::Command { command, env } => {
            let (first, rest) = command
                .split_first()
                .map_or(("", &[][..]), |(first, rest)| (first.as_str(), rest));
            How::Spawn {
                program: program(first, dir),
                args: rest.to_vec(),
                env: resolve(env),
            }
        }
        Reach::Url {
            url,
            bearer,
            headers,
        } => {
            let bearer = bearer
                .as_ref()
                .map(|bearer| ("Authorization".to_string(), bearer.clone()));
            let mut headers = resolve(
                &bearer
                    .into_iter()
                    .chain(headers.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            if let Some((_, token)) = headers
                .first_mut()
                .filter(|(name, _)| name == "Authorization")
            {
                *token = format!("Bearer {token}");
            }
            How::Connect {
                url: url.clone(),
                headers,
                login: !super::login::explicit(server)
                    && super::login::stored(&config, &server.name).is_some(),
            }
        }
    };
    Plan {
        how,
        dir: work_dir(core, &server.name),
        log: stderr_file(core, &server.name),
        server: server.clone(),
        known: None,
    }
}

/// 一个值：字照原样，引用的照这一刻的配置取（密钥文件、环境变量），取不到的没有。
fn resolved(config: &crate::config::Config, value: &EnvValue) -> Option<String> {
    match value {
        EnvValue::Text(text) => Some(text.clone()),
        EnvValue::Reference(reference) => config
            .secret(reference)
            .map(|secret| secret.expose().to_string()),
    }
}

/// 服务 `service` 的标准错误的文件：`state/logs/mcp-<服务名>.stderr`（软件包列表上停下了的给最后几行，施工 X-4）。
pub(crate) fn stderr_file(core: &Core, service: &str) -> PathBuf {
    core.root
        .state()
        .join("logs")
        .join(format!("mcp-{service}.stderr"))
}

/// 服务 `service` 的工作目录：`state/mcp/<服务名>/`。
pub(super) fn work_dir(core: &Core, service: &str) -> PathBuf {
    core.root.state().join("mcp").join(service)
}

/// 只有名字的命令照这几种文件名找：Windows 上 `npx`、`uvx` 这类是 `.cmd`，`Command` 自己只认 `.exe`（施工 X-5）。找到的
/// `.cmd`、`.bat` 照完整路径起，标准库经 `cmd.exe` 转、参数照它的规矩转义。别的平台只找名字本身。
#[cfg(windows)]
const SUFFIXES: &[&str] = &[".exe", ".cmd", ".bat"];
#[cfg(not(windows))]
const SUFFIXES: &[&str] = &[""];

/// 命令第一项 `first` 指的程序：绝对路径的照它；带路径分隔符的照包目录 `dir` 算；只有名字的先找包目录的 `bin/`（包自己带的
/// 程序放那里，施工 F-8 上再补），再照 `PATH` 找（`npx`、`uvx` 这类）。别的平台照 `PATH` 找交给 `Command`（它查可执行权限），
/// 交回名字本身；Windows 上它只认 `.exe`，自己照 [`SUFFIXES`] 找，找不到的交回名字本身，起的时候报找不到。
pub(super) fn program(first: &str, dir: &Path) -> PathBuf {
    let path_dirs: Vec<PathBuf> = match std::env::var_os("PATH") {
        Some(path) if cfg!(windows) => std::env::split_paths(&path).collect(),
        _ => Vec::new(),
    };
    find(first, dir, &path_dirs, SUFFIXES)
}

/// [`program`] 本身：`path_dirs` 是 `PATH` 里的目录，`suffixes` 是照着找的几种文件名后缀（测试里两种平台的都试）。
fn find(first: &str, dir: &Path, path_dirs: &[PathBuf], suffixes: &[&str]) -> PathBuf {
    let path = Path::new(first);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if first.contains('/') || first.contains(std::path::MAIN_SEPARATOR) {
        return dir.join(path);
    }
    let bin = dir.join(BIN);
    let places = dir
        .is_absolute()
        .then_some(bin.as_path())
        .into_iter()
        .chain(path_dirs.iter().map(PathBuf::as_path));
    for place in places {
        for suffix in suffixes {
            let candidate = place.join(format!("{first}{suffix}"));
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    path.to_path_buf()
}

/// 起：程序正被开着写的等 [`BUSY_WAIT`] 再试，最多 [`BUSY_TRIES`] 次；别的错误照旧交回。网址的服务不起进程，交回出错（调的一方先照
/// [`How`] 分开走，不会走到这里）。
pub(super) async fn spawn(plan: &Plan) -> io::Result<Child> {
    let mut tries = 1;
    loop {
        match spawn_once(plan) {
            Err(error)
                if error.kind() == io::ErrorKind::ExecutableFileBusy && tries < BUSY_TRIES =>
            {
                tries += 1;
                tokio::time::sleep(BUSY_WAIT).await;
            }
            done => return done,
        }
    }
}

/// 起一次：工作目录没有的建（只在数据根里建，施工 TMP-1），标准错误接到它的文件上，核心丢下它时杀掉。
fn spawn_once(plan: &Plan) -> io::Result<Child> {
    let How::Spawn { program, args, env } = &plan.how else {
        return Err(io::Error::other("a URL server has no process"));
    };
    miyu_store::root::create_inside(&plan.dir)?;
    let errors = stderr::open(&plan.log)?;
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .envs(env.iter().map(|(name, value)| (name, value)))
        .current_dir(&plan.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(errors);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut command = Command::from(command);
    command.kill_on_drop(true);
    command.spawn()
}

#[cfg(test)]
mod tests;
