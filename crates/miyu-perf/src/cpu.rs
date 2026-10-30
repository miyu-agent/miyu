//! 读一个进程花了多少 CPU（施工 V-22 起的 `schedstat`，施工 V-29 加 cgroup）：只有 Linux 上读得到，别的平台是 `None`。
//!
//! 两个口径：
//! - 进程自己：各线程 `/proc/<进程号>/task/<线程号>/schedstat` 头一格（在 CPU 上的纳秒）加起来。一段时间里退了的线程算不进
//!   （阻塞线程空闲 10 秒才退，量的那一段里退不了），拉起的子进程不算。
//! - 连子进程（施工 V-29）：核心放进自己的 systemd scope 跑（`Sandbox::start` 的 `scoped`），读那个 cgroup 的
//!   `cpu.stat` 的 `usage_usec`：核心、它拉起的 `miyu-sandbox`、`bwrap`、命令本身，退出了的也算。挪到子进程里的活照样
//!   算进来（2026-10-30 项目主人定：挪到子进程要算总账）。

use std::path::{Path, PathBuf};

/// 要读谁：核心的进程号，和它所在的 cgroup（没放进自己的 scope 的是 `None`）。
#[derive(Debug, Clone)]
pub struct Probe {
    /// 核心的进程号。
    pub pid: u32,
    /// 核心自己那个 scope 的 cgroup 目录（`/sys/fs/cgroup/...`）。
    pub group: Option<PathBuf>,
}

/// 某一刻读到的累计 CPU（毫秒）。读不到的那一格是 `None`。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cpu {
    /// 核心进程各线程加起来。
    pub threads: Option<f64>,
    /// 核心那个 cgroup 加起来，含子进程。
    pub group: Option<f64>,
}

impl Cpu {
    /// 从 `before` 到这一刻花了多少：两头都读到的那一格才有。
    pub fn since(&self, before: &Cpu) -> Cpu {
        let minus = |a: Option<f64>, b: Option<f64>| Some(a? - b?);
        Cpu {
            threads: minus(self.threads, before.threads),
            group: minus(self.group, before.group),
        }
    }
}

impl Probe {
    /// 现在读一次。
    pub fn sample(&self) -> Cpu {
        Cpu {
            threads: threads(self.pid),
            group: self.group.as_deref().and_then(group),
        }
    }
}

/// 进程 `pid` 各线程花的 CPU（毫秒）。
pub fn threads(pid: u32) -> Option<f64> {
    let tasks = std::fs::read_dir(Path::new("/proc").join(pid.to_string()).join("task")).ok()?;
    let mut nanos: u64 = 0;
    for task in tasks.flatten() {
        // 线程刚好退了的读不到：跳过它，不让整次作废。
        let Ok(text) = std::fs::read_to_string(task.path().join("schedstat")) else {
            continue;
        };
        nanos += text.split_whitespace().next()?.parse::<u64>().ok()?;
    }
    Some(nanos as f64 / 1_000_000.0)
}

/// cgroup 目录 `dir` 的 `cpu.stat` 里的 `usage_usec`，折成毫秒。
fn group(dir: &Path) -> Option<f64> {
    let text = std::fs::read_to_string(dir.join("cpu.stat")).ok()?;
    usage(&text)
}

/// 从 `cpu.stat` 的字里读出 `usage_usec`，折成毫秒。
pub fn usage(text: &str) -> Option<f64> {
    text.lines()
        .filter_map(|line| line.split_once(' '))
        .find(|(name, _)| *name == "usage_usec")
        .and_then(|(_, value)| value.trim().parse::<u64>().ok())
        .map(|micros| micros as f64 / 1000.0)
}

/// 进程 `pid` 所在的 cgroup 目录：读 `/proc/<进程号>/cgroup` 里 cgroup v2 那一行（`0::<路径>`）。不是 v2 的是 `None`。
pub fn group_of(pid: u32) -> Option<PathBuf> {
    let text =
        std::fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("cgroup")).ok()?;
    let path = unified(&text)?;
    let dir = Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'));
    dir.join("cpu.stat").exists().then_some(dir)
}

/// `/proc/<进程号>/cgroup` 里 cgroup v2 那一行的路径。
pub fn unified(text: &str) -> Option<&str> {
    text.lines().find_map(|line| line.strip_prefix("0::"))
}

#[cfg(test)]
mod tests;
