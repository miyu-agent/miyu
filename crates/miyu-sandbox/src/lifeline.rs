//! 核心没了，它起的命令跟着没（施工 7-8，`internal/blueprint/core.md`「子进程随核心退出」，`12-进程形态与分发.md` R5）。
//! 有计划地停下时核心自己整组杀掉后台命令（施工 7-3）；这里管的是核心崩了、被 `SIGKILL`、被任务管理器结束的时候：
//! 没人来杀，命令得自己知道核心没了。三个平台各用自己最稳的现成办法：
//!
//! - Unix（Linux、macOS 一样）：一根「生命线」管道，写端只在核心手里（`O_CLOEXEC`，别的子进程拿不到），核心怎么没的，
//!   内核都会关掉它。每条命令的进程组里先起一个看门的，标准输入是读端：读到结尾（核心没了），杀掉自己所在的整个组
//!   （`kill(0, SIGKILL)`）。命令起在这个组里，前台、后台、经不经沙盒的助手都一样（`Watcher`）。看门的只认自己的组，
//!   组里还有它就不会被别的组占了编号，所以不会杀错。看门的是主程序真实位置旁边的小程序 `miyu-watchdog`（施工 LW-28；
//!   V-48 到 LW-28 之间照沙盒的助手起，比 `/bin/sh` 起得快，每条命令省约 0.3 ms 的 CPU）；找不到的、起不来的退回
//!   `/bin/sh`，跑 `while read -r _; do :; done; kill -s KILL 0`，做的是同一件事。
//! - Windows：核心起来时把自己放进一个带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的作业对象（[`bind_children`]），之后起的
//!   子进程、子进程再起的都在里面；核心没了，作业的最后一个句柄跟着关，里面的全部结束。
//!
//! 核心起的长住的子进程（扩展、命令型 MCP 服务、语义模型小程序，施工 XH-1）照同一个办法跟着核心走（[`keep`]）：Unix 上共用
//! 一个看门的、一个组（施工 LW-28），Windows 上照继承在核心的作业对象里。
//!
//! 没用 Linux 的 `PR_SET_PDEATHSIG`：它盯的是起子进程的那个线程，线程池的线程一退，命令就被误杀；也只管直接的子进程，组里
//! 别的照样活。没用 macOS 的 kqueue：要另起一个进程盯着，和管道一样，却只有一个平台能用。

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{Lifeline, WATCHDOG, Watcher, watchdog_beside, watcher};
#[cfg(windows)]
pub use windows::Job;

/// 一个长住的子进程拿着的共用的组（[`keep`]）：拿到子进程退出、收掉以后再丢。丢掉时组里的都不杀（有计划地停照旧由调的一方
/// 先请它退出、等、再杀）；最后一个拿着的丢了，共用的看门的跟着收掉。看门的起不来的、不是 Unix 的是空的（[`Kept::default`]）。
#[derive(Debug, Default)]
pub struct Kept {
    /// 共用的组：只拿着，丢掉时放开。
    #[cfg(unix)]
    _share: Option<unix::Share>,
}

#[cfg(all(unix, any(test, feature = "testkit")))]
impl Kept {
    /// 子进程起在哪个组里；空的没有。只有测试问（`testkit` 特性）。
    pub fn group(&self) -> Option<u32> {
        self._share.as_ref().map(unix::Share::group)
    }

    /// 这一刻看着这个组的看门的的进程号；空的没有。只有测试问（`testkit` 特性）。
    pub fn watchdog(&self) -> Option<u32> {
        self._share.as_ref().map(unix::Share::watchdog)
    }
}

/// 让核心起的长住的子进程跟着核心走（施工 XH-1）：Unix 上 `command` 起在这个进程的生命线上共用的那个组里（施工 LW-28，
/// Unix 的 `Lifeline::keep`）：头一次先起一个看门的（同 `shell` 的命令，照 `miyu-watchdog` 起、退回 `/bin/sh`），之后的都进这个组；
/// 核心怎么没的，看门的都把整组杀掉，所有长住的子进程和它们起的、还在组里的孙进程都在里面。Windows 上核心起来时已经进了
/// 作业对象（[`bind_children`]），子进程照继承在里面，这里什么都不做。
///
/// # Errors
///
/// 看门的起不来（Unix）：`command` 照旧自成一组（同 `shell` 的命令），调的一方记一行 `WARN`、照旧起，交回的空 [`Kept`] 用
/// [`Kept::default`]。
#[cfg_attr(
    not(unix),
    expect(unused_variables, reason = "别的平台不用改起法：照继承在作业对象里")
)]
pub fn keep(command: &mut std::process::Command) -> std::io::Result<Kept> {
    #[cfg(unix)]
    return unix::keep(command);
    #[cfg(not(unix))]
    Ok(Kept::default())
}

/// 把核心自己放进一个作业对象，核心没了，它起的子进程都跟着结束（Windows）。核心起来时调一次。别的平台什么都不做：
/// 那里靠每条命令组里看门的（`watcher`）。
///
/// # Errors
///
/// 作业对象建不成、设不上、放不进去：交回系统的原话，调的一方记一行运行日志，照旧起来。
pub fn bind_children() -> std::io::Result<()> {
    #[cfg(windows)]
    return windows::bind_children();
    #[cfg(not(windows))]
    Ok(())
}
