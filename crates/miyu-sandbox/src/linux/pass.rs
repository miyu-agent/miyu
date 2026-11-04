//! 缓存的规则集怎么交给助手（施工 V-47）：核心里的规则集带 `O_CLOEXEC`，起别的子进程时都不会漏过去；起助手时只在这一个
//! 子进程里（`fork` 以后、`exec` 以前）去掉它的 `O_CLOEXEC`，号码写在助手的参数里（`--ruleset <号码>`）。助手先给它设回
//! `O_CLOEXEC`（之后不管成不成，都不会漏给命令），再叠一小层管连 Unix 套接字的、收紧自己、关掉它，最后才换成命令：命令
//! 拿不到它，往里加不了规则（加了就放宽了以后每一条命令）。
#![expect(
    unsafe_code,
    reason = "起子进程时去掉一个描述符的 O_CLOEXEC（pre_exec），助手照号码收紧自己（prctl、landlock_restrict_self、close）：标准库、landlock 库都没有现成的安全接口"
)]

use std::io;
use std::os::fd::{AsFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;

use crate::Spec;

use super::rules::{self, Layer};

/// 让 `command` 起的子进程拿到规则集 `fd`（只这一个子进程，`exec` 时不关），交回它的号码。`command` 留着它，起完了才放。
///
/// 起子进程要走 `fork`（标准库有 `pre_exec` 就不用 `posix_spawn`）。
pub(crate) fn inherit(command: &mut Command, fd: Arc<OwnedFd>) -> RawFd {
    use std::os::fd::AsRawFd;
    let raw = fd.as_raw_fd();
    let keep = move || -> io::Result<()> {
        // 子进程里：只去掉这一个描述符的 `FD_CLOEXEC`。`fcntl` 是异步信号安全的系统调用，不分配内存、不拿锁。
        rustix::io::fcntl_setfd(fd.as_fd(), rustix::io::FdFlags::empty()).map_err(io::Error::from)
    };
    // SAFETY: 闭包在 `fork` 出来的子进程里、`exec` 以前跑，只调一次 `fcntl`（异步信号安全，不分配、不拿锁）；`fd` 由闭包拿着、
    // 活到 `command` 丢掉，子进程里只借用它的号码。
    unsafe {
        command.pre_exec(keep);
    }
    raw
}

/// 助手照核心交来的规则集收紧自己（施工 V-47）：先给 `ruleset` 设回 `O_CLOEXEC`；建好每条命令另叠的那一小层（`Layer::Unix`：
/// 连 Unix 套接字、跨目录改名和链接，只放 `write`、`/dev/null`、`sockets`；这台内核管不了连套接字的不叠，从头建的那一层本来也
/// 不管）；设 `no_new_privs`；照 `ruleset` 收紧（`landlock_restrict_self`），关掉它；再照那一小层收紧。
///
/// # Errors
///
/// `ruleset` 不是打开着的描述符、建不成那一小层、收不住：说原话（`landlock: …`、`cannot open <路径>: …`、
/// `landlock is not available: …`）。
pub fn confine_cached(spec: &Spec, ruleset: RawFd) -> Result<(), String> {
    // SAFETY: 只设一个描述符的标志，没有指针；号码不是打开着的回 EBADF，照出错说。
    if unsafe { libc::fcntl(ruleset, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
        return Err(format!(
            "landlock: bad ruleset {ruleset}: {}",
            io::Error::last_os_error()
        ));
    }
    let unix = match rules::unix_supported() {
        true => Some(rules::build(rules::ruleset(Layer::Unix)?, &rules::unix(spec), None)?.0),
        false => None,
    };
    // SAFETY: 只设一个进程标志，没有指针。
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(format!(
            "landlock: failed to set no_new_privs: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: 两个整数参数，没有指针；内核查 `ruleset` 是不是规则集，不是的回 EBADFD。
    if unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0_u32) } != 0 {
        return Err(format!(
            "landlock: failed to restrict the calling thread: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: 关掉核心交来、只有助手用的那一个；之后不再用这个号码。
    unsafe {
        libc::close(ruleset);
    }
    match unix {
        Some(unix) => rules::restrict(unix),
        None => Ok(()),
    }
}
