//! 怎么起一个服务（施工 X-3 上，`mcp.md`「怎么走」第二条第 2、5 条）：程序照命令第一项找，工作目录是状态目录里它的一格，
//! 环境是核心的加上清单写的，标准错误接到日志目录里它的一份。不进沙盒。

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

use miyu_config::package::{EnvValue, McpServer};
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
    /// 程序：找到的位置，只有名字的照 `PATH` 找。
    pub(super) program: PathBuf,
    /// 参数：命令第一项后面的。
    pub(super) args: Vec<String>,
    /// 另给的环境变量，引用的已经取出来了。
    pub(super) env: Vec<(String, String)>,
    /// 工作目录：`state/mcp/<服务名>/`，没有的建。
    pub(super) dir: PathBuf,
    /// 标准错误接到哪个文件。
    pub(super) log: PathBuf,
}

/// 包 `package`（包目录 `dir`）带的服务 `server` 怎么起：引用的密钥、环境变量照这一刻的配置取，取不到的那一个不设、记一行
/// `WARN`。
pub(super) fn plan(core: &Core, package: &str, server: &McpServer, dir: &Path) -> Plan {
    let config = core.config_now().borrow().clone();
    let mut env = Vec::new();
    for (name, value) in &server.env {
        match value {
            EnvValue::Text(text) => env.push((name.clone(), text.clone())),
            EnvValue::Reference(reference) => match config.secret(reference) {
                Some(secret) => env.push((name.clone(), secret.expose().to_string())),
                None => {
                    tracing::warn!(target: TARGET, package, service = server.name.as_str(), variable = name.as_str(), reference = reference.name(), "mcp env not set");
                }
            },
        }
    }
    let (first, rest) = server
        .command
        .split_first()
        .map_or(("", &[][..]), |(first, rest)| (first.as_str(), rest));
    Plan {
        program: program(first, dir),
        args: rest.to_vec(),
        env,
        dir: core.root.state().join("mcp").join(&server.name),
        log: core
            .root
            .state()
            .join("logs")
            .join(format!("mcp-{}.stderr", server.name)),
        server: server.clone(),
    }
}

/// 命令第一项 `first` 指的程序：绝对路径的照它；带路径分隔符的照包目录 `dir` 算；只有名字的先找包目录的 `bin/`（包自己带的
/// 程序放那里，施工 F-8 上再补），没有的交回名字本身，起的时候照 `PATH` 找（`npx`、`uvx` 这类）。
pub(super) fn program(first: &str, dir: &Path) -> PathBuf {
    let path = Path::new(first);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if first.contains('/') || first.contains(std::path::MAIN_SEPARATOR) {
        return dir.join(path);
    }
    let bundled = dir
        .join(BIN)
        .join(format!("{first}{}", std::env::consts::EXE_SUFFIX));
    if dir.is_absolute() && bundled.is_file() {
        return bundled;
    }
    path.to_path_buf()
}

/// 起：程序正被开着写的等 [`BUSY_WAIT`] 再试，最多 [`BUSY_TRIES`] 次；别的错误照旧交回。
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

/// 起一次：工作目录没有的建，标准错误接到它的文件上，核心丢下它时杀掉。
fn spawn_once(plan: &Plan) -> io::Result<Child> {
    std::fs::create_dir_all(&plan.dir)?;
    let errors = stderr::open(&plan.log)?;
    let mut command = std::process::Command::new(&plan.program);
    command
        .args(&plan.args)
        .envs(plan.env.iter().map(|(name, value)| (name, value)))
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
