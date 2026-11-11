//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：只用 Landlock（施工 5-2 起；5-3 照 DeepSeek 的 dsh 改成整盘能
//! 读、只管写）。不要命名空间、不要 AppArmor（2026-09-29 项目主人定）。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：「除了藏起来的都能读」是一级级放行做出来的（库里的
//! `miyu_sandbox::linux`，施工 V-47 从这里挪过去，核心缓存规则集也用它）；能写的整个放，不在里面挖洞（施工 5-14 改，照 dsh：
//! 藏不住的会话那边不写进 `hidden`；施工 5-15 到 5-14 改之间一级级绕开过，冻住了路上那几层）。核心交来缓存的规则集的
//! （`--ruleset`，施工 V-47）照它收紧（[`run_ruled`]）。施工 5-16 有过用 bubblewrap 的挂载收紧的一路，施工 LV-5 去掉了
//! （设计 11 的 A17）：藏起来的目录看得到名字、读不到里面。

mod seccomp;

use miyu_sandbox::linux as landlock;

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。
///
/// 关起来的会话的套接字怎么管（施工 MB-1）：Landlock 管得住的报 `landlock-sockets`，管不住、有 seccomp 过滤模式的报
/// `seccomp`；两样都没有的不报，核心不让关起来的会话跑命令。
pub(crate) fn mechanisms() -> Vec<String> {
    if landlock::available().is_err() {
        return Vec::new();
    }
    let mut found = vec!["landlock".to_string()];
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

/// 照核心交来的规则集（描述符 `ruleset`，施工 V-47）收紧，再换成那条命令。只有没写 `read` 的规格才有：写了 `read` 的
/// 不跑（核心不会这样交）。收不住的说原话，退出 125，不跑命令。
pub(crate) fn run_ruled(
    spec: &Spec,
    ruleset: std::os::fd::RawFd,
    program: &OsStr,
    args: &[OsString],
) -> ExitCode {
    if spec.read.is_some() {
        return crate::fail("bad spec: --ruleset with read");
    }
    if let Err(why) = landlock::confine_cached(spec, ruleset) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}
