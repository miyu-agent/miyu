//! 查包收尾删解开的目录（施工 SD-2补）。rc.4 的 Windows 那台删 `.check` 碰上 os error 32（文件被别的进程占着）：
//!
//! - Windows 上跑着的程序、它打开的文件都删不掉；
//! - 查包跑了包里的 `miyu`。`miyu core stop` 等核心放开锁才回，但锁放了到进程真没了之间还有一小段，核心带起来的进程也一样；
//! - 也可能是杀毒软件之类在扫刚解开的程序。
//!
//! 所以先等：程序文件在这个目录里、还在跑的进程，有限时间内等它们退，等过谁印在标准错误上（CI 日志里看得到根因）；还没退的
//! 报出是谁，不删。都退了再删；删的时候碰上 Windows 的「被占着」（32）、「拒绝访问」（5），退避着重试几次，只当兜底。Linux、macOS
//! 上跑着的程序照样删得掉，直接删。

use std::path::Path;
use std::time::Duration;

use super::super::layout::remove;

/// 等那几个进程退最多等多久。
const WAIT: Duration = Duration::from_secs(30);

/// 先等目录里的程序都退了，再删目录；没有的不算错。
pub(super) fn remove_settled(dir: &Path) -> Result<(), String> {
    remove_within(dir, WAIT)
}

/// 同 [`remove_settled`]，最多等 `wait`。
pub(super) fn remove_within(dir: &Path, wait: Duration) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    if cfg!(windows) {
        wait_for_programs(dir, wait)?;
        retried(dir)
    } else {
        remove(dir)
    }
}

/// 程序文件在 `dir` 里、还在跑的进程都退了才回；等过的印一行。过了 `wait` 还有的报出它们。
fn wait_for_programs(dir: &Path, wait: Duration) -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut seen = Vec::new();
    loop {
        let running = programs_under(dir);
        if running.is_empty() {
            if !seen.is_empty() {
                eprintln!(
                    "查包：等了 {:.1} 秒，这几个进程退了才删 {}：{}",
                    started.elapsed().as_secs_f64(),
                    dir.display(),
                    seen.join("；")
                );
            }
            return Ok(());
        }
        for one in &running {
            if !seen.contains(one) {
                seen.push(one.clone());
            }
        }
        if started.elapsed() >= wait {
            return Err(format!(
                "{} 里的程序等了 {} 秒还在跑，没删：{}",
                dir.display(),
                wait.as_secs(),
                running.join("；")
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// 删，碰上「被占着」「拒绝访问」的退避着重试，最后还不成的说清楚占着它的不是这个目录里的程序。
fn retried(dir: &Path) -> Result<(), String> {
    let mut pause = Duration::from_millis(200);
    let mut last = String::new();
    for _ in 0..8 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) if matches!(error.raw_os_error(), Some(5 | 32)) => {
                last = error.to_string();
                eprintln!(
                    "查包：{} 删不掉（{error}），{} 毫秒后再试",
                    dir.display(),
                    pause.as_millis()
                );
                std::thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_secs(2));
            }
            Err(error) => return Err(format!("{}：{error}", dir.display())),
        }
    }
    Err(format!(
        "{}：{last}（试了几次；里面的程序都没在跑，占着它的是别的进程，例如杀毒软件）",
        dir.display()
    ))
}

/// 程序文件在 `dir` 里、还在跑的进程：`<进程号> <程序>`。只有 Windows 上问（PowerShell 的 Win32_Process），问不成的当没有。
fn programs_under(dir: &Path) -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let prefix =
        format!("{}\\", dir.display().to_string().trim_end_matches('\\')).replace('\'', "''");
    let script = format!(
        "Get-CimInstance Win32_Process | Where-Object {{ $_.ExecutablePath -and \
         $_.ExecutablePath.StartsWith('{prefix}', [StringComparison]::OrdinalIgnoreCase) }} | \
         ForEach-Object {{ '{{0}} {{1}}' -f $_.ProcessId, $_.ExecutablePath }}"
    );
    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
    {
        Ok(done) if done.status.success() => String::from_utf8_lossy(&done.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
