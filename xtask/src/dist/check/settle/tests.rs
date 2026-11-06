//! 删的时候里面有程序在跑：把这个测试程序自己拷进临时目录，从那里起一个睡一会儿的（[`a_program_that_sleeps`]）。Windows 上
//! 删一个跑着的程序删不掉，正是 rc.4 碰上的；Linux、macOS 上照样删得掉，只查删干净了。

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use super::*;

/// 起出来的那一份照这个环境变量睡多少毫秒。
const SLEEP: &str = "MIYU_XTASK_TEST_SLEEP_MS";

/// 给下面几条当「目录里跑着的程序」用，平时不跑。
#[test]
#[ignore = "只给别的测试当子进程用"]
fn a_program_that_sleeps() {
    let millis = std::env::var(SLEEP)
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(0);
    std::thread::sleep(Duration::from_millis(millis));
}

/// 新的临时目录，名字带 `name` 和进程号。放在这个测试程序旁边（施工 FL-7）：和它在同一个文件系统上，[`sleeper`] 才连得成
/// 硬链接（本机 `/tmp` 是 tmpfs、`target` 在别的文件系统上，放 `/tmp` 会一直退回拷）。
fn scratch(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("知道自己在哪");
    let beside = exe.parent().expect("测试程序在某个目录里");
    let dir = beside.join(format!("xtask-settle-{name}-{}", std::process::id()));
    remove_settled(&dir).expect("清得掉上次留下的");
    std::fs::create_dir_all(&dir).expect("建得了");
    dir
}

/// 把这个测试程序放进 `dir` 叫 `sleeper`，从那里起一份睡 `millis` 毫秒。
///
/// Unix 上用硬链接、连不成才拷（施工 FL-7）：`fs::copy` 拷的时候开着写的句柄，同一个测试程序里别的线程这时起子进程，子进程在
/// exec 以前也带着这个句柄；接着 exec 刚拷好的文件，Linux 回 ETXTBSY（「Text file busy」）。硬链接不开写的句柄，没有这个窗口。
/// Windows 照旧拷：没有 ETXTBSY，那几条测试要的是「目录里有一份正在跑的程序」。
fn sleeper(dir: &Path, millis: u64) -> Child {
    let program = dir.join(format!("sleeper{}", std::env::consts::EXE_SUFFIX));
    let exe = std::env::current_exe().expect("知道自己在哪");
    if cfg!(windows) || std::fs::hard_link(&exe, &program).is_err() {
        std::fs::copy(&exe, &program).expect("拷得了");
    }
    let child = Command::new(&program)
        .args([
            "--ignored",
            "--exact",
            "dist::check::settle::tests::a_program_that_sleeps",
        ])
        .env(SLEEP, millis.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("起得来");
    // 等它真起来、把程序文件映射上。
    std::thread::sleep(Duration::from_millis(500));
    child
}

#[test]
fn a_running_program_inside_is_waited_for_then_removed() {
    let dir = scratch("waits");
    let mut child = sleeper(&dir, 3000);
    let started = Instant::now();
    remove_settled(&dir).expect("等它退了删得掉");
    assert!(!dir.exists(), "删干净了");
    if cfg!(windows) {
        assert!(
            started.elapsed() >= Duration::from_secs(1),
            "Windows 上要等程序退了才删得掉：{:?}",
            started.elapsed()
        );
    }
    child.wait().expect("收得了");
}

#[test]
fn a_missing_dir_is_fine() {
    let dir = std::env::temp_dir().join(format!("xtask-settle-none-{}", std::process::id()));
    remove_settled(&dir).expect("没有的不算错");
}

#[cfg(windows)]
#[test]
fn a_program_that_stays_is_named_and_nothing_is_removed() {
    let dir = scratch("stays");
    let mut child = sleeper(&dir, 60_000);
    let error = remove_within(&dir, Duration::from_secs(2)).expect_err("还在跑的不删");
    assert!(error.contains("sleeper"), "报出是谁：{error}");
    assert!(dir.join("sleeper.exe").exists(), "没删");
    child.kill().expect("停得了");
    child.wait().expect("收得了");
    remove_settled(&dir).expect("退了以后删得掉");
}
