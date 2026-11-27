//! 前台壳（施工 CS-11，`core.md`「起来的先后」开头）：人在终端前台直接跑 `miyu core`（这个进程有控制终端）时，这个进程不当
//! 核心，只当一个很小的壳：把真正的核心拉起到一个新会话里（同头拉起核心，施工 CS-10 的 `new_session`），参数、环境变量原样
//! 带过去，标准输出、标准错误还接着这个终端，标准输入接空；之后挂着终端等它。真正的核心没有控制终端，它起的 `zsh -i` 这类打不开
//! `/dev/tty`、抢不走前台；V-48 的看门做法一条不动，两个平台一个写法。
//!
//! - 判据：能打开 `/dev/tty` 就算有控制终端（`miyu_pipe::controlling_terminal`）。头拉起的、`miyu core start` 拉起的、systemd、
//!   launchd 起的核心都没有，不走这里；壳拉起的那一个也没有，不会再套一层。
//! - 壳收到 `SIGINT`（终端里按 Ctrl+C，信号只到前台组，也就是壳）、`SIGTERM`、`SIGHUP`（关掉终端时 shell 转来）、`SIGQUIT`，
//!   原样转给核心。处理在拉起核心以前就装好，中间来的不会照默认把壳杀掉、把核心丢下。
//! - 核心退出，壳照它的退出码退出；核心被信号杀掉的照 shell 的写法交 128 加信号号。
//! - 壳不开库、不拿锁、不装运行日志：数据根的锁照常由真正的核心拿，日志照常写进它的 `core.log`。
//! - 壳自己被 `SIGKILL` 了：核心不跟着走，同头拉起的核心（头没了核心照旧跑，等别的头来连）。
//!
//! Windows 上没有这一层：头拉起时 `DETACHED_PROCESS`，在控制台里直接跑的命令没有「抢前台」这回事。

use std::process::ExitCode;

/// 这个进程有控制终端的（人在终端前台直接跑 `miyu core`），当前台壳跑完，交回退出码；没有的交回 `None`，调的一方照常当核心。
pub fn foreground() -> Option<ExitCode> {
    #[cfg(unix)]
    if miyu_pipe::controlling_terminal() {
        return Some(unix::shell());
    }
    None
}

#[cfg(unix)]
mod unix {
    use std::io::{self, Write};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, ExitCode, ExitStatus, Stdio};

    use rustix::process::{Pid, Signal, kill_process};
    use tokio::signal::unix::{SignalKind, signal};
    use tokio::sync::oneshot;

    /// 拉起核心、转信号、等它退出（见这一页开头）。起不来的在标准错误上说一句，退出码 1。
    pub(super) fn shell() -> ExitCode {
        match run() {
            Ok(code) => code,
            Err(error) => {
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "标准错误写不了的，没有别的地方可以说"
                )]
                let _ = writeln!(io::stderr().lock(), "miyu: core not started: {error}");
                ExitCode::FAILURE
            }
        }
    }

    /// 同上，出错的交回原话。
    fn run() -> io::Result<ExitCode> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            // 先装好再拉起：中间来的信号不会照默认把壳杀掉、把核心丢下。
            let mut interrupt = signal(SignalKind::interrupt())?;
            let mut terminate = signal(SignalKind::terminate())?;
            let mut hangup = signal(SignalKind::hangup())?;
            let mut quit = signal(SignalKind::quit())?;
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args(std::env::args_os().skip(1))
                .stdin(Stdio::null());
            miyu_pipe::new_session(&mut command);
            let mut child = command.spawn()?;
            let pid = i32::try_from(child.id())
                .ok()
                .and_then(Pid::from_raw)
                .ok_or_else(|| io::Error::other("core pid out of range"))?;
            let (exited, mut gone) = oneshot::channel();
            std::thread::spawn(move || drop(exited.send(child.wait())));
            loop {
                let forwarded = tokio::select! {
                    status = &mut gone => {
                        return status.map_err(io::Error::other)?.map(code);
                    }
                    _ = interrupt.recv() => Signal::INT,
                    _ = terminate.recv() => Signal::TERM,
                    _ = hangup.recv() => Signal::HUP,
                    _ = quit.recv() => Signal::QUIT,
                };
                // 核心这一刻刚好退了、还没收到它的退出码的，转不过去也不要紧：下一轮就收到了。
                match kill_process(pid, forwarded) {
                    Ok(()) | Err(_) => {}
                }
            }
        })
    }

    /// 核心的退出码：被信号杀掉的照 shell 的写法交 128 加信号号。
    fn code(status: ExitStatus) -> ExitCode {
        match (status.code(), status.signal()) {
            (Some(code), _) => ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1)),
            (None, Some(signal)) => ExitCode::from(u8::try_from(128 + signal).unwrap_or(1)),
            (None, None) => ExitCode::FAILURE,
        }
    }
}
