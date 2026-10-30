//! 交给 MXC 的禁止路径先挑一遍（施工 SW-1，`docs/blueprint/sandbox/windows-run.md`「怎么走」第 2 条第 4 款）：MXC 把禁止路径
//! 原样交给系统的 `CreateProcessSecurityEnvironment`，系统要一条条打开它们；正被别的进程独占打开的文件（用户登录着时的
//! `NTUSER.DAT`、`UsrClass.dat`）打不开，整个安全环境建不成（`0x80070020` 共享冲突），这一条命令就跑不了。这样的文件谁都
//! 读不到，跳过它不少挡什么，照样跑；目录、不在的、别的错照旧交上去。
//!
//! 挑法是纯逻辑（[`deniable`]，哪个系统都测）；「是不是被别的进程独占着」只在 Windows 上问（`busy`）。

use std::path::{Path, PathBuf};

/// `hidden` 里去掉 `busy` 说正被别的进程独占着的，先后不变。
pub fn deniable(hidden: &[PathBuf], busy: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    hidden.iter().filter(|path| !busy(path)).cloned().collect()
}

/// `path` 是不是正被别的进程独占打开的文件：照常打开（读、写、删都让人共享）报共享冲突（32）、锁冲突（33）的算。目录照常
/// 打不开（要另带标志），报的是拒绝访问，不算；不在的、别的错也不算。
#[cfg(windows)]
pub fn busy(path: &Path) -> bool {
    /// `ERROR_SHARING_VIOLATION`、`ERROR_LOCK_VIOLATION`。
    const HELD: [i32; 2] = [32, 33];
    match std::fs::File::open(path) {
        Ok(_) => false,
        Err(error) => error
            .raw_os_error()
            .is_some_and(|code| HELD.contains(&code)),
    }
}

#[cfg(test)]
mod tests;
