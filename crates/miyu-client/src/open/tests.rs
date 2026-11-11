//! 把网址交给系统（施工 WB-1）：拉起来的程序当场退了的照退出码算，没退的不等它，当交出去了。

#![cfg(unix)]

use std::process::Command;
use std::time::{Duration, Instant};

use super::handed;

/// 跑一句 shell。
fn sh(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", script]);
    command
}

#[test]
fn an_opener_that_stays_is_not_waited_for() {
    // 浏览器没在跑时 `xdg-open` 换成浏览器本身、不退：原来一直等到它退，`miyu web` 挂着，托盘等满 90 秒报「打开网页失败」。
    let started = Instant::now();
    assert!(handed(sh("sleep 20"), Duration::from_millis(300)));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "没等它退：{:?}",
        started.elapsed()
    );
}

#[test]
fn an_opener_that_fails_at_once_did_not_hand_over() {
    assert!(!handed(sh("exit 3"), Duration::from_secs(10)));
    let missing = Command::new("/nonexistent/miyu-no-such-opener");
    assert!(!handed(missing, Duration::from_secs(10)));
}

#[test]
fn an_opener_that_returns_at_once_handed_over() {
    // 浏览器已经开着的那种：转交了马上退，不等满。
    let started = Instant::now();
    assert!(handed(sh("exit 0"), Duration::from_secs(10)));
    assert!(started.elapsed() < Duration::from_secs(5));
}
