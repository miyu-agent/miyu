//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：只用 Landlock（施工 5-2 起；5-3 照 DeepSeek 的 dsh 改成整盘能
//! 读、只管写）。不要命名空间、不要 AppArmor（2026-09-29 项目主人定）。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：「除了藏起来的都能读」是一级级放行做出来的（[`reads`]）；能写的
//! 整个放，不在里面挖洞（施工 5-14 改，照 dsh：藏不住的会话那边不写进 `hidden`；施工 5-15 到 5-14 改之间一级级绕开过，
//! 冻住了路上那几层）。有能用的 bubblewrap 时规格写着 `mount`，用挂载盖住（[`bwrap`]，施工 5-16）。

mod aliases;
mod bwrap;
mod landlock;
mod reads;
mod seccomp;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。有能用的 bubblewrap 的多一个 `bwrap`（施工 5-16）：它要在里面再用 Landlock，
/// 所以只在 Landlock 也能用时报。
///
/// 关起来的会话的套接字怎么管（施工 MB-1）：Landlock 管得住的报 `landlock-sockets`，管不住、有 seccomp 过滤模式的报
/// `seccomp`；两样都没有的不报，核心不让关起来的会话跑命令。
pub(crate) fn mechanisms() -> Vec<String> {
    if landlock::available().is_err() {
        return Vec::new();
    }
    let mut found = vec!["landlock".to_string()];
    if bwrap::usable() {
        found.push("bwrap".to_string());
    }
    if landlock::confines_sockets() {
        found.push("landlock-sockets".to_string());
    } else if seccomp::available() {
        found.push("seccomp".to_string());
    }
    found
}

/// 强制走 seccomp 的环境变量（施工 MB-1，测试在新内核上量老内核的那一路）：写成 `force` 的照 seccomp 挡，只会更严。
const FORCE_SECCOMP: &str = "MIYU_SANDBOX_SECCOMP";

/// 照规格 `spec` 收紧，再换成那条命令。收不住的说原话，退出 125，不跑命令。
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    // 规格要挂载的换成 bubblewrap，挂好以后在里面再跑一次自己（施工 5-16）。
    if spec.mount {
        return bwrap::exec(spec, program, args);
    }
    // 关起来的会话（写了 `read`）：Landlock 管不住连 Unix 套接字的，再照 seccomp 不准新建（施工 MB-1）。先问再收紧：收紧以后
    // 建不了规则集。
    let seccomp = spec.read.is_some()
        && (std::env::var_os(FORCE_SECCOMP).is_some_and(|value| value == "force")
            || !landlock::confines_sockets());
    if let Err(why) = landlock::confine(spec) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    if seccomp && let Err(why) = seccomp::deny_unix_sockets() {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}
