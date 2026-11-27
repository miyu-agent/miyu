//! 一次停的信号就退（施工 CS-8，`internal/blueprint/core.md`「停下」第 4 条）：真的主程序、真的核心，Unix 上测。
//!
//! 基准测试的测具和多数服务管理器一样只发一次 `SIGTERM`，等几秒不走就 `SIGKILL`。这里看两种以前一次停不下的：起来那一段还没说
//! `ready` 的（系统配置换成命名管道，核心卡在装好停的信号以后读整份配置那一步），说了 `ready`、目录还在读的（缓存目录里的目录换成
//! 命名管道，读目录的线程卡在打开它上面）。

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::stopping::exit_within;
use crate::support::{Home, count};
use crate::waiting::{core_pid, feed, signal, sockets, stuck_config};

/// 起来那一段（还没说 `ready`）收到一次停的信号：当场退，退出码 0，运行日志记一行「停了」，带 `ready=false`（修之前留到开始
/// 接连接时才停，卡在起来的一直不走，2026-11-27 基准测试 B1 第 1 次）。
#[test]
fn one_stop_signal_ends_a_core_still_starting() {
    let home = Home::new();
    let fifo = stuck_config(&home);
    let mut core = home
        .core_staying()
        .stdout(Stdio::null())
        .spawn()
        .expect("起得来");
    let pid = core_pid(&home);
    // 第一次读配置（建运行时以前数工作线程）放过去，卡在第二次：那时停的信号已经装好、套接字也绑好了。
    feed(&fifo);
    let sockets = sockets(&home);
    let until = Instant::now() + Duration::from_secs(60);
    while !sockets.iter().any(|socket| socket.exists()) {
        assert!(Instant::now() < until, "没建套接字：{}", home.core_log());
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(200));
    signal(&pid, "TERM");
    let status = exit_within(&mut core, Duration::from_secs(10));
    feed(&fifo);
    let log = home.core_log();
    assert_eq!(status.and_then(|status| status.code()), Some(0), "{log}");
    assert_eq!(count(&log, "stopped reason=signal ready=false"), 1, "{log}");
    assert_eq!(count(&log, "listening"), 0, "没说好了：{log}");
}

/// 说了 `ready`、目录还在读的时候收到一次停的信号：照常收尾，退出码 0，不等读目录的线程（修之前收尾要等它读完，压力下读一份
/// 目录一两分钟，只有收尾的总时限到了才强退，2026-11-27 基准测试 B1 第 5 次）。
#[test]
fn one_stop_signal_ends_a_core_still_reading_the_catalog() {
    let home = Home::new();
    // 缓存目录在测试的临时目录里：Linux 照 `XDG_CACHE_HOME`（`offline`），macOS 照家目录，这里把家目录也指进来。
    let user = home.root.path().join("var/cache/test-cache/home");
    let models = if cfg!(target_os = "macos") {
        user.join("Library/Caches/Miyu/models")
    } else {
        home.root.path().join("var/cache/test-cache/miyu/models")
    };
    std::fs::create_dir_all(&models).expect("建得了");
    // 缓存的比快照新：先读它，读的线程卡在打开命名管道上。
    std::fs::write(
        models.join("models-dev.meta.json"),
        r#"{"source": "test", "fetched": "2099-01-01T00:00:00.000Z"}"#,
    )
    .expect("写得进");
    let fifo = models.join("models-dev.json");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("跑得了");
    assert!(made.success(), "建得了命名管道");
    let mut core = home
        .core_staying()
        .env("HOME", &user)
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "目录不挡 ready：{}", home.core_log());
    signal(&core.id().to_string(), "TERM");
    let status = exit_within(&mut core, Duration::from_secs(10));
    let log = home.core_log();
    assert_eq!(status.and_then(|status| status.code()), Some(0), "{log}");
    assert_eq!(count(&log, "stopped reason=signal"), 1, "{log}");
    assert_eq!(count(&log, "catalog loaded"), 0, "那时目录还在读：{log}");
}
