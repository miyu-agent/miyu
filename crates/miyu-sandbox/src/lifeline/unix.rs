//! Unix 上的生命线（`lifeline.rs`）：一根管道，一个组一个看门的。看门的照沙盒的助手起（`miyu-sandbox lifeline`，施工
//! V-48），起不来的退回 `/bin/sh`。

use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

/// 退回 `/bin/sh` 时看门的那一段：读到结尾（生命线的写端全关了）就杀掉自己所在的整个组，自己也在里面。`read` 和
/// `kill` 都是内建的，不经 `PATH`；没人往管道里写，`read` 一直等到结尾。助手的 `lifeline` 做的是同一件事。
const SCRIPT: &str = "while read -r _; do :; done; kill -s KILL 0";

/// 退回时看门的 shell：各个 Unix 都有，不经 `PATH` 找。
const SHELL: &str = "/bin/sh";

/// 助手看门时的子命令（`src/bin/miyu-sandbox/lifeline.rs`）。
pub const MODE: &str = "lifeline";

/// 核心探过、和自己同一份源码编的助手（[`use_helper`]）：看门的照它起。没设过的直接起 `/bin/sh`。
static HELPER: OnceLock<PathBuf> = OnceLock::new();

/// 看门的照这个助手起（施工 V-48）：核心探完沙盒、助手和自己是同一份源码编的（构建 id 对得上）时设一次，之后起的看门的都
/// 用它；再设的不算。
pub fn use_helper(helper: &Path) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "设过的不再换：一个进程里只探一次，同一个助手"
    )]
    let _ = HELPER.set(helper.to_path_buf());
}

/// 一根生命线：写端拿在手里，谁都不给；读端交给每个看门的。
#[derive(Debug)]
pub struct Lifeline {
    reader: io::PipeReader,
    /// 只拿着，不写：它关了（这个进程没了，或者丢掉了这根线），看门的读到结尾。
    _writer: io::PipeWriter,
}

impl Lifeline {
    /// 一根新的生命线。两头都带 `O_CLOEXEC`（标准库建的管道都是）：起别的子进程时不会漏给它们。
    ///
    /// # Errors
    ///
    /// 管道建不起来（文件描述符用完了之类）。
    pub fn new() -> io::Result<Lifeline> {
        let (reader, writer) = io::pipe()?;
        Ok(Lifeline {
            reader,
            _writer: writer,
        })
    }

    /// 起一个看门的，自成一组（组号就是它的进程号）：命令照 [`Watcher::group`] 起在这个组里。给了助手 `helper` 的照它的
    /// `lifeline` 起；没给的、起不来的（文件没了、执行不了）起 `/bin/sh`。两样起法一样：环境变量清空、工作目录是 `/`、
    /// 标准输入是生命线的读端、标准输出和错误接空的，别的文件描述符一个都不带（标准库建的都带 `O_CLOEXEC`）。
    ///
    /// # Errors
    ///
    /// 读端复制不了，或者 `/bin/sh` 也起不来。
    pub fn watcher(&self, helper: Option<&Path>) -> io::Result<Watcher> {
        if let Some(helper) = helper
            && let Ok(child) = self.spawn(Command::new(helper).arg(MODE))
        {
            return Ok(Watcher {
                child,
                helper: true,
            });
        }
        let mut shell = Command::new(SHELL);
        shell.args(["-c", SCRIPT]);
        Ok(Watcher {
            child: self.spawn(&mut shell)?,
            helper: false,
        })
    }

    /// 照看门的起法起 `command`。
    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        command
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::from(self.reader.try_clone()?))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
    }
}

/// 这个进程的那一根：第一次要用时建，一直拿到进程结束。建不起来的，之后每次都说建不起来。
fn lifeline() -> io::Result<&'static Lifeline> {
    static LIFELINE: OnceLock<Result<Lifeline, String>> = OnceLock::new();
    LIFELINE
        .get_or_init(|| Lifeline::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| io::Error::other(error.clone()))
}

/// 在这个进程的生命线上起一个看门的（[`Lifeline::watcher`]）：核心设过助手的（[`use_helper`]）照助手起。
///
/// # Errors
///
/// 生命线建不起来，或者看门的起不来。
pub fn watcher() -> io::Result<Watcher> {
    lifeline()?.watcher(HELPER.get().map(PathBuf::as_path))
}

/// 让 `command` 起在一个新看门的组里（施工 XH-1，`lifeline.rs` 的 `keep`）。看门的起不来的，`command` 照旧自成一组，交回
/// 起不来的原话。
pub(super) fn keep(command: &mut Command) -> io::Result<Watcher> {
    command.process_group(0);
    let watcher = watcher()?;
    let group = i32::try_from(watcher.group()).map_err(io::Error::other)?;
    command.process_group(group);
    Ok(watcher)
}

/// 一个组里看门的。整组杀的时候它跟着没；丢掉它时杀掉它、收掉它，不留僵尸进程。
#[derive(Debug)]
pub struct Watcher {
    child: Child,
    /// 照助手起的；退回 `/bin/sh` 的是 `false`。
    helper: bool,
}

impl Watcher {
    /// 它的组：命令起在这个组里（`process_group`），整组杀也照它（`kill(-组号)`）。
    pub fn group(&self) -> u32 {
        self.child.id()
    }

    /// 是不是照助手起的：退回 `/bin/sh` 的不是。
    pub fn by_helper(&self) -> bool {
        self.helper
    }
}

impl Drop for Watcher {
    /// 杀掉、收掉。它还没被收过，编号还是它的，按编号杀不会杀错；已经跟着整组没了的，杀它是空的，收它马上回来。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "已经没了的杀不了、收不了都不要紧：只为不留僵尸进程"
    )]
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
