//! 把一个描述符交给子进程、放到固定编号上（设计 39 第五节第 3 条，原型 M-0）。
//!
//! 核心这一头建的描述符带 `CLOEXEC`（标准库默认），别的会话同一刻起的子进程拿不到；只在要起的那个子进程里，fork 之后、
//! exec 之前 `dup2` 到固定编号，`dup2` 出来的不带 `CLOEXEC`，一路传到 shell 和它起的命令。要写 `pre_exec`，是 `unsafe`，
//! 所以放在这里（工作区别处一律 `forbid`）。

use std::os::fd::RawFd;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// 起 `command` 时把描述符 `fd` 放到子进程的 `target` 上。调的一方要让 `fd` 一直开着，直到起完。
pub fn pass_fd(command: &mut Command, fd: RawFd, target: RawFd) {
    #[allow(unsafe_code)]
    // SAFETY: 闭包在 fork 之后、exec 之前跑，只调 `dup2`、`fcntl` 两个异步信号安全的系统调用，不分配内存、不拿锁。
    unsafe {
        command.pre_exec(move || {
            if fd == target {
                // 编号碰巧一样：`dup2` 什么都不做，`CLOEXEC` 还在，要自己清掉。
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            } else if libc::dup2(fd, target) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
