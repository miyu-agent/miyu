//! seccomp（施工 MB-1，2026-10-24 主会话定）：内核的 Landlock 管不住连 Unix 套接字（第 9 版起才管得住连路径上的）、抽象
//! 套接字（第 6 版起）的机器上，关起来的会话（规格写了 `read`）的命令不准新建 AF_UNIX 套接字：`socket(AF_UNIX, …)` 回
//! `EACCES`。路径上的、抽象的都连不上：机主的 D-Bus 会话总线、Docker、ssh-agent 这类能替命令在沙盒外读写的系统服务挡住。
//! `socketpair` 照放（Python 的多进程、node 的管道要它，连不到外面）。域名解析照常：nss 连 systemd-resolved 的套接字连不上，
//! 回退到 DNS。
//!
//! 过滤只认这个架构的系统调用号：x86_64 上 32 位的（i386）照它自己的号挡 `socket`、整个挡 `socketcall`（参数在内存里，
//! 看不到是哪一种），x32 的（号带 `0x40000000`）一律回 `EACCES`；aarch64 上 32 位的（arm）同样照它的号。认不出的架构一律
//! 回 `EACCES`。

#![expect(
    unsafe_code,
    reason = "调 prctl 装 seccomp 过滤、探内核有没有过滤模式，每一处写明为什么安全"
)]

/// `socket` 的第一个参数：Unix 域。
const AF_UNIX: u32 = 1;

/// 回给命令的错误码。
const EACCES: u32 = 13;

/// 放行。
const RET_ALLOW: u32 = 0x7fff_0000;

/// 回错误码（低 16 位是错误码）。
const RET_ERRNO: u32 = 0x0005_0000;

/// `seccomp_data` 里的几格：系统调用号、架构、第一个参数的低 32 位（小端）。
const NR: u32 = 0;
const ARCH: u32 = 4;
const ARG0: u32 = 16;

/// BPF 的几种指令：照绝对位置读 32 位、相等就跳、大于等于就跳、交回。
const LD_ABS: u16 = 0x20;
const JEQ: u16 = 0x15;
const JGE: u16 = 0x35;
const RET: u16 = 0x06;

/// x32 的系统调用号带的那一位。
const X32: u32 = 0x4000_0000;

/// 这个架构自己的：审计用的架构号、`socket` 的号。
#[cfg(target_arch = "x86_64")]
const NATIVE: (u32, u32) = (0xC000_003E, 41);
#[cfg(target_arch = "aarch64")]
const NATIVE: (u32, u32) = (0xC000_00B7, 198);

/// 同一台机器上 32 位的：审计用的架构号、`socketcall` 的号、`socket` 的号。
#[cfg(target_arch = "x86_64")]
const COMPAT: (u32, u32, u32) = (0x4000_0003, 102, 359);
#[cfg(target_arch = "aarch64")]
const COMPAT: (u32, u32, u32) = (0x4000_0028, 102, 281);

/// 一条指令。
fn op(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

/// 过滤：跳的远近照下面的行号数（目标 = 这一行 + 1 + 跳的格数）。
fn program() -> Vec<libc::sock_filter> {
    let deny = RET_ERRNO | EACCES;
    vec![
        /* 0 */ op(LD_ABS, 0, 0, ARCH),
        /* 1 */ op(JEQ, 0, 3, NATIVE.0), // 不是这个架构的去 5
        /* 2 */ op(LD_ABS, 0, 0, NR),
        /* 3 */ op(JGE, 8, 0, X32), // x32 的去 12
        /* 4 */ op(JEQ, 4, 6, NATIVE.1), // socket 去 9，别的去 11
        /* 5 */ op(JEQ, 0, 6, COMPAT.0), // 认不出的架构去 12
        /* 6 */ op(LD_ABS, 0, 0, NR),
        /* 7 */ op(JEQ, 4, 0, COMPAT.1), // socketcall 去 12
        /* 8 */ op(JEQ, 0, 2, COMPAT.2), // socket 去 9，别的去 11
        /* 9 */ op(LD_ABS, 0, 0, ARG0),
        /* 10 */ op(JEQ, 1, 0, AF_UNIX), // Unix 域的去 12
        /* 11 */ op(RET, 0, 0, RET_ALLOW),
        /* 12 */ op(RET, 0, 0, deny),
    ]
}

/// 内核有没有 seccomp 的过滤模式：照 libseccomp、Chromium 的探法，交一个空指针给 `PR_SET_SECCOMP`，有过滤模式的回
/// `EFAULT`（指针不对），没有的回 `EINVAL`。什么都不装，探测的进程不受影响。
pub(super) fn available() -> bool {
    // SAFETY: 第三个参数是空指针，内核只检查它、不往里写；有过滤模式的在读它之前回 EFAULT，什么都没装上。
    let result = unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            std::ptr::null::<libc::sock_fprog>(),
            0,
            0,
        )
    };
    result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EFAULT)
}

/// 装上过滤：之后这个进程和它起的命令都不能新建 AF_UNIX 套接字。先设 `no_new_privs`（Landlock 收紧时设过，这里再设一次不
/// 依赖它）。
pub(super) fn deny_unix_sockets() -> Result<(), String> {
    let filter = program();
    let len = u16::try_from(filter.len()).map_err(|_| "seccomp: filter too long".to_string())?;
    let program = libc::sock_fprog {
        len,
        filter: filter.as_ptr().cast_mut(),
    };
    // SAFETY: 只设一个进程标志，没有指针。
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(format!("seccomp: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: `program` 指向 `filter` 里的那几条，两样都活到这次调用结束；内核在调用里把过滤拷走，之后不再读它们。
    let result = unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            std::ptr::from_ref(&program),
            0,
            0,
        )
    };
    if result != 0 {
        return Err(format!("seccomp: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
