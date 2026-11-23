//! 看门的小程序 `miyu-watchdog`（施工 LW-28，`internal/blueprint/core.md`「子进程随核心退出」，生命线在
//! `miyu_sandbox::lifeline`）。核心在一个进程组里先起它，标准输入是核心那根生命线的读端：读到结尾（核心没了，写端被内核关掉）
//! 就杀掉自己所在的整个组，自己也在里面（`kill(0, SIGKILL)`）。和退回用的 `/bin/sh -c 'while read -r _; do :; done; kill -s
//! KILL 0'` 做同一件事：读到的东西不管，读不了（出错）也当结尾。
//!
//! 不带参数。带了参数的不看门，在标准错误上说一句用法、退出 125（同沙盒的助手的用法错）：认错了真去看门，杀的是起它的那一方
//! 的组。要起得轻：不碰环境变量、不经标准库带缓冲的标准输入（那一份要分配 8 KiB），栈上一小块读。
//!
//! 只在 Unix 上看门。Windows 上核心进作业对象（`miyu_sandbox::lifeline::bind_children`），用不到它：那里编出来的程序只说一句、
//! 退出 125，发行包里不带。

use std::io::Write;
use std::process::ExitCode;

/// 用法错、杀不成时的退出码，同沙盒的助手自己出错的 125（`env`、`timeout` 的约定）。
const EXIT_USAGE: u8 = 125;

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        return fail(
            "usage: miyu-watchdog (no arguments; reads stdin to the end, then kills its process group)",
        );
    }
    #[cfg(unix)]
    return watch();
    #[cfg(not(unix))]
    fail("only watches on Unix")
}

/// 等到生命线断了，杀整组。杀成了就回不来；杀不成（不该有）退出 125，看门的就没了，组里的照常跑，和 `/bin/sh` 那条一样。
#[cfg(unix)]
fn watch() -> ExitCode {
    use rustix::io::{Errno, read};
    use rustix::process::{Signal, kill_current_process_group};

    let stdin = rustix::stdio::stdin();
    let mut buffer = [0u8; 64];
    loop {
        match read(stdin, &mut buffer) {
            Ok(0) => break,
            Ok(_) | Err(Errno::INTR) => {}
            Err(_) => break,
        }
    }
    match kill_current_process_group(Signal::KILL) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(EXIT_USAGE),
    }
}

/// 在标准错误上说一句（带上自己的名字），退出 125。
fn fail(what: &str) -> ExitCode {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "标准错误写不了的，没有别的地方可以说"
    )]
    let _ = writeln!(std::io::stderr().lock(), "miyu-watchdog: {what}");
    ExitCode::from(EXIT_USAGE)
}
