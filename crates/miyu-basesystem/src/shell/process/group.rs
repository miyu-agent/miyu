//! 整组杀（施工 4-8，从 `process.rs` 挪出来，施工 7-12 那边放不下了）：Unix 上杀进程组，Windows 上 `taskkill /T` 杀整棵
//! 进程树；掐掉这次调用时整组杀的 [`Guard`]。

#[cfg(windows)]
use std::process::{Command, Stdio};

#[cfg(windows)]
use super::CREATE_NO_WINDOW;
use super::TARGET;

/// 一条命令和它起的：Unix 上是它的进程组，Windows 上是它的进程树。认的是命令的进程编号。
#[derive(Debug, Clone, Copy)]
pub(in crate::shell) struct Group(pub(super) u32);

impl Group {
    /// 整组杀掉。
    pub(in crate::shell) fn kill(self) {
        kill(self.0);
    }

    /// 命令退出以后，组里还在跑的也杀掉。Windows 上不杀：退出以后按编号杀可能杀错。
    pub(in crate::shell) fn leftovers(self) {
        if cfg!(unix) {
            kill(self.0);
        }
    }
}

/// Unix：`kill(-pgid, SIGKILL)`。组里已经没有进程的，不算出错。
#[cfg(unix)]
fn kill(id: u32) {
    use rustix::process::{Pid, Signal, kill_process_group};
    let Some(pid) = i32::try_from(id).ok().and_then(Pid::from_raw) else {
        return;
    };
    match kill_process_group(pid, Signal::KILL) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::SRCH => {}
        Err(error) => tracing::warn!(target: TARGET, error = %error, "command group not killed"),
    }
}

/// Windows：`taskkill /T /F /PID`，整棵进程树。
#[cfg(windows)]
fn kill(id: u32) {
    use std::os::windows::process::CommandExt;
    let killed = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &id.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    if let Err(error) = killed {
        tracing::warn!(target: TARGET, error = %error, "command tree not killed");
    }
}

/// 别的系统：只能杀它自己，这里拿不到它，不杀。
#[cfg(not(any(unix, windows)))]
fn kill(_id: u32) {}

/// 掐掉时整组杀掉：丢掉它就杀；跑完了先 [`Guard::disarm`]。掐掉就是丢掉这次调用的 future（施工 4-2）；执行命令
/// 不会被「叫它停」，那只给改文件的（施工 4-9 再补一）。
pub(in crate::shell) struct Guard(Option<Group>);

impl Guard {
    /// 看着 `group`。
    pub(in crate::shell) fn new(group: Group) -> Guard {
        Guard(Some(group))
    }

    /// 跑完了，丢掉它也不杀。
    pub(in crate::shell) fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(group) = self.0.take() {
            group.kill();
        }
    }
}
