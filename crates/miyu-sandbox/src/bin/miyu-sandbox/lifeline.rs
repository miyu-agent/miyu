//! 看门（施工 V-48，`miyu_sandbox::lifeline`）：`miyu-sandbox lifeline`。核心给每条命令的进程组先起一个它，标准输入是核心
//! 那根生命线的读端：读到结尾（核心没了，写端被内核关掉）就杀掉自己所在的整个组，自己也在里面（`kill(0, SIGKILL)`）。
//! 和原来退回用的 `/bin/sh -c 'while read -r _; do :; done; kill -s KILL 0'` 做同一件事：读到的东西不管，读不了（出错）
//! 也当结尾。
//!
//! 要起得轻：不读规格、不收紧、不碰环境变量，不经标准库带缓冲的标准输入（那一份要分配 8 KiB），栈上一小块读。

use std::os::fd::AsFd;
use std::process::ExitCode;

use rustix::io::{Errno, read};
use rustix::process::{Signal, kill_current_process_group};

/// 等到生命线断了，杀整组。杀成了就回不来；杀不成（不该有）退出 125，看门的就没了，命令照常跑，和 `/bin/sh` 那条一样。
pub(crate) fn watch() -> ExitCode {
    let stdin = std::io::stdin();
    let mut buffer = [0u8; 64];
    loop {
        match read(stdin.as_fd(), &mut buffer) {
            Ok(0) => break,
            Ok(_) | Err(Errno::INTR) => {}
            Err(_) => break,
        }
    }
    match kill_current_process_group(Signal::KILL) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(miyu_sandbox::EXIT_HELPER),
    }
}
