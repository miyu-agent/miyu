//! 别的核心进程留下的目录（施工 M-3 上，WC-1 起 Windows 也清）：临时目录下一个核心进程一个 `miyu-call-<进程号>`，放套接字（Unix）
//! 和 `PATH` 里那个 `miyu`。核心没地方在走时删它（Windows 上 `bin\miyu.exe` 是正在跑的核心程序的硬链接，删不掉），所以第一次用时
//! 顺手删掉进程已经不在了的：两边共用这一条规矩，问进程在不在、目录是不是自己的照平台给。

use std::fs::Metadata;
use std::path::Path;

use super::serve::TARGET;

/// 目录名的前缀，后面跟核心的进程号。
pub(super) const PREFIX: &str = "miyu-call-";

/// 删掉临时目录 `temp` 下进程已经不在了的 `miyu-call-<进程号>`：名字对得上（前缀后面全是数字）、是目录（不是链接）、`ours` 说是
/// 自己的、`alive` 说那个进程不在了的才删。删不掉的记一行 `debug`，下次再删。
pub(super) fn sweep(temp: &Path, alive: impl Fn(u32) -> bool, ours: impl Fn(&Metadata) -> bool) {
    let Ok(entries) = std::fs::read_dir(temp) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(pid_of) else {
            continue;
        };
        // 不跟链接：叫这个名字的链接不是我们建的。
        let ours = entry
            .metadata()
            .is_ok_and(|meta| meta.is_dir() && ours(&meta));
        if ours
            && !alive(pid)
            && let Err(error) = std::fs::remove_dir_all(entry.path())
        {
            tracing::debug!(target: TARGET, error = %error, "stale miyu call directory not removed");
        }
    }
}

/// `miyu-call-<进程号>` 里的进程号：前缀后面全是十进制数字、放得进 `u32` 的才算（`+1`、`-1` 这样的不算）。
fn pid_of(name: &str) -> Option<u32> {
    let pid = name.strip_prefix(PREFIX)?;
    if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

#[cfg(test)]
mod tests;
