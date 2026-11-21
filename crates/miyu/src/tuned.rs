//! 核心带着「线程缓存少留」的那一项环境变量起来（施工 LW-12，`internal/blueprint/heap.md`「线程缓存」）：这个参数 glibc 只在进程
//! 起来时读，进程里改不了。
//!
//! - `miyu` 自己拉起核心时（界面顺带拉起、`miyu core start`）设在命令上（[`on`]），核心一起来就是调好的。
//! - 别的办法起的核心（systemd 这类托管的、人直接运行 `miyu core`）没带着：进核心之前把自己重新执行一遍、带上
//!   （[`relaunch`]）。重新执行不换进程号，标准输入输出照旧，托管的一方看到的还是同一个进程。带上了就不再重新执行。

use std::ffi::OsString;
use std::process::Command;

use miyu_heap::launch;

/// 这个进程起来时要补上的那几项；用不着的（不是 glibc、已经带着、人自己写了线程缓存留几块）是空的。
fn missing() -> Vec<(&'static str, OsString)> {
    launch::vars(std::env::var_os(launch::TUNABLES).as_deref())
}

/// 拉起核心的命令 `core` 带上那几项。
pub(crate) fn on(core: &mut Command) {
    core.envs(missing());
}

/// 前台跑核心之前调：没带着那几项的，照原样的参数重新执行自己、带上；换成了就不回来。换不成的（找不到自己在哪、系统不让）
/// 照原样往下跑，只是线程缓存照 glibc 默认的留，核心起来那一行 `heap tuned` 的 `tunables` 里没有那一项。
pub(crate) fn relaunch() {
    let vars = missing();
    if !vars.is_empty() {
        again(vars);
    }
}

/// 带着 `vars` 重新执行自己：换成了不回来。
#[cfg(unix)]
fn again(vars: Vec<(&'static str, OsString)>) {
    use std::os::unix::process::CommandExt;
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    // 回来了就是没换成：原因不在这里说，核心起来那一行如实记没调上。
    drop(
        Command::new(exe)
            .args(std::env::args_os().skip(1))
            .envs(vars)
            .exec(),
    );
}

/// 没有「换成另一个程序」这回事的平台：这里用的也不是 glibc，走不到。
#[cfg(not(unix))]
fn again(_: Vec<(&'static str, OsString)>) {}
