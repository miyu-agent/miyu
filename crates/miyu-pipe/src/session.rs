//! 拉起的子进程开一个新会话（Unix，施工 CS-10）：子进程 `fork` 以后、`exec` 以前调 `setsid()`，自己当会话和进程组的头，
//! 不带控制终端。拉起的一方的终端它碰不到：打不开 `/dev/tty`（`ENXIO`），`tcsetpgrp` 抢不了前台，终端里按 `Ctrl+C`、关掉
//! 终端也打不到它。标准库能开新会话的那个开关（`CommandExt::setsid`）还没稳定，要自己在 `pre_exec` 里调，所以放在这个放开了
//! `unsafe` 的小 crate 里。
//!
//! `setsid()` 要求调的一方不是进程组的头：同一个 `Command` 不能再设 `process_group`（标准库先 `setpgid`、后跑 `pre_exec`，
//! 设了的话 `setsid()` 回 `EPERM`，子进程起不来）。同一个 `Command` 调了两次的（测试夹具先设了一次，拉起核心的 `detach` 又设一次，
//! 施工 CS-11）：第二次 `setsid()` 也回 `EPERM`，这时自己已经是会话的头，当成了。开了新会话，进程组号就是它的进程号，照它整组杀
//! 照旧（`kill(-pid)`）。

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// 让 `command` 起来时开一个新会话（见这一页开头）：不带控制终端，进程组号就是它的进程号。`setsid()` 不成、自己又不是会话的头
/// 的（`command` 设了 `process_group`），起的时候 `spawn` 交回那个错。
pub fn new_session(command: &mut Command) {
    // SAFETY: 闭包在 `fork` 出来的子进程里、`exec` 以前跑，那里只能调异步信号安全的函数。它只调 `setsid()`、`getsid()`、
    // `getpid()`（都是 POSIX 列出的异步信号安全函数，一次系统调用），出错时把错误码包成 `io::Error`（只存一个整数，不分配内存、
    // 不加锁），不碰别的。
    unsafe {
        command.pre_exec(|| match rustix::process::setsid() {
            Ok(_) => Ok(()),
            Err(_) if leading() => Ok(()),
            Err(error) => Err(io::Error::from_raw_os_error(error.raw_os_error())),
        });
    }
}

/// 自己已经是会话的头。
fn leading() -> bool {
    rustix::process::getsid(None).is_ok_and(|sid| sid == rustix::process::getpid())
}

#[cfg(test)]
mod tests;
