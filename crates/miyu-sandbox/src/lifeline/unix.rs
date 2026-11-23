//! Unix 上的生命线（`lifeline.rs`）：一根管道；命令一个组一个看门的，长住的子进程共用一个看门的（`shared.rs`）。看门的照主程序
//! 旁边的 `miyu-watchdog` 起（施工 LW-28），起不来的退回 `/bin/sh`。

mod shared;

use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use super::Kept;
pub(in crate::lifeline) use shared::Share;

/// 退回 `/bin/sh` 时看门的那一段：读到结尾（生命线的写端全关了）就杀掉自己所在的整个组，自己也在里面。`read` 和
/// `kill` 都是内建的，不经 `PATH`；没人往管道里写，`read` 一直等到结尾。`miyu-watchdog` 做的是同一件事。
const SCRIPT: &str = "while read -r _; do :; done; kill -s KILL 0";

/// 退回时看门的 shell：各个 Unix 都有，不经 `PATH` 找。
const SHELL: &str = "/bin/sh";

/// 看门的小程序的文件名（施工 LW-28，`crates/miyu-watchdog`）：只在 Unix 上有。
pub const WATCHDOG: &str = "miyu-watchdog";

/// 主程序的真实位置是 `exe`，旁边的看门的小程序（同沙盒的助手的 [`crate::locate()`]）。没有的、不是普通文件的，是空的。
pub fn watchdog_beside(exe: &Path) -> Option<PathBuf> {
    let watchdog = exe.parent()?.join(WATCHDOG);
    watchdog.is_file().then_some(watchdog)
}

/// 这个进程的看门的小程序：第一次要用时照这个程序的真实位置找一次（[`watchdog_beside`]），之后一直用它。找不到的（测试程序、
/// 只拷了主程序的）是空的，看门的都照 `/bin/sh` 起。看门的做的事是固定的，不用和核心对构建 id；升级换了程序以后照旧起旁边那一个。
fn watchdog() -> Option<&'static Path> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let exe = std::env::current_exe().and_then(std::fs::canonicalize);
            exe.ok().and_then(|exe| watchdog_beside(&exe))
        })
        .as_deref()
}

/// 一根生命线：写端拿在手里，谁都不给；读端交给每个看门的。长住的子进程共用的那一个看门的也记在这根线上（[`Lifeline::keep`]）。
#[derive(Debug)]
pub struct Lifeline {
    reader: io::PipeReader,
    /// 只拿着，不写：它关了（这个进程没了，或者丢掉了这根线），看门的读到结尾。
    _writer: io::PipeWriter,
    /// 长住的子进程共用的组（`shared.rs`）。
    shared: shared::Slot,
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
            shared: shared::Slot::default(),
        })
    }

    /// 起一个看门的，自成一组（组号就是它的进程号）：命令照 [`Watcher::group`] 起在这个组里。给了 `watchdog` 的照它起；没给的、
    /// 起不来的（文件没了、执行不了）起 `/bin/sh`。两样起法一样：环境变量清空、工作目录是 `/`、标准输入是生命线的读端、标准
    /// 输出和错误接空的，别的文件描述符一个都不带（标准库建的都带 `O_CLOEXEC`）。
    ///
    /// # Errors
    ///
    /// 读端复制不了，或者 `/bin/sh` 也起不来。
    pub fn watcher(&self, watchdog: Option<&Path>) -> io::Result<Watcher> {
        self.watcher_in(watchdog, 0)
    }

    /// 同 [`Lifeline::watcher`]，起在组 `group` 里（`0` 是自成一组）。组没了（`setpgid` 不成）的两样都起不来，交回原话。
    fn watcher_in(&self, watchdog: Option<&Path>, group: i32) -> io::Result<Watcher> {
        if let Some(watchdog) = watchdog
            && let Ok(child) = self.spawn(&mut Command::new(watchdog), group)
        {
            return Ok(Watcher {
                child,
                #[cfg(any(test, feature = "testkit"))]
                by_watchdog: true,
            });
        }
        let mut shell = Command::new(SHELL);
        shell.args(["-c", SCRIPT]);
        Ok(Watcher {
            child: self.spawn(&mut shell, group)?,
            #[cfg(any(test, feature = "testkit"))]
            by_watchdog: false,
        })
    }

    /// 照看门的起法起 `command`，放进组 `group`。
    fn spawn(&self, command: &mut Command, group: i32) -> io::Result<Child> {
        command
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::from(self.reader.try_clone()?))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(group)
            .spawn()
    }

    /// 让长住的子进程 `command` 起在这根线上共用的那个看门的组里（施工 LW-28，`lifeline.rs` 的 `keep`）：还没有、看门的没了的
    /// 先起一个（照 `watchdog`，同 [`Lifeline::watcher`]）。交回的 [`Kept`] 拿到子进程退出、收掉以后再丢。
    ///
    /// # Errors
    ///
    /// 看门的起不来：`command` 自成一组，交回起不来的原话。
    pub fn keep(&self, watchdog: Option<&Path>, command: &mut Command) -> io::Result<Kept> {
        command.process_group(0);
        let share = self.shared.keep(|group| self.watcher_in(watchdog, group))?;
        command.process_group(i32::try_from(share.group()).map_err(io::Error::other)?);
        Ok(Kept {
            _share: Some(share),
        })
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

/// 在这个进程的生命线上起一个看门的（[`Lifeline::watcher`]），照主程序旁边的 `miyu-watchdog` 起。
///
/// # Errors
///
/// 生命线建不起来，或者看门的起不来。
pub fn watcher() -> io::Result<Watcher> {
    lifeline()?.watcher(watchdog())
}

/// 让 `command` 起在这个进程的生命线上共用的那个组里（[`Lifeline::keep`]，`lifeline.rs` 的 `keep`）。
pub(super) fn keep(command: &mut Command) -> io::Result<Kept> {
    match lifeline() {
        Ok(lifeline) => lifeline.keep(watchdog(), command),
        Err(error) => {
            command.process_group(0);
            Err(error)
        }
    }
}

/// 一个组里看门的。整组杀的时候它跟着没；丢掉它时杀掉它、收掉它，不留僵尸进程。
#[derive(Debug)]
pub struct Watcher {
    child: Child,
    /// 照 `miyu-watchdog` 起的；退回 `/bin/sh` 的是 `false`。只有测试问（[`Watcher::by_watchdog`]）。
    #[cfg(any(test, feature = "testkit"))]
    by_watchdog: bool,
}

impl Watcher {
    /// 它的组：命令起在这个组里（`process_group`），整组杀也照它（`kill(-组号)`）。命令的看门的自成一组，组号就是它的进程号；
    /// 长住的子进程共用的组另记组号（`unix/shared.rs`：看门的换过的，进程号和组号不一样）。
    pub fn group(&self) -> u32 {
        self.child.id()
    }

    /// 是不是照 `miyu-watchdog` 起的：退回 `/bin/sh` 的不是。只有测试问（`testkit` 特性）。
    #[cfg(any(test, feature = "testkit"))]
    pub fn by_watchdog(&self) -> bool {
        self.by_watchdog
    }

    /// 它已经没了：没了的这一下收掉（`try_wait`），之后按编号杀、收都是空的，不会打到别人。问不了的也当没了。
    fn gone(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
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
