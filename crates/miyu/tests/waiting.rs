//! 等核心（施工 CS-6，`internal/blueprint/ipc.md`「等多久」）：真的主程序、真的核心，Unix 上测。
//!
//! 核心起得慢造法有两种：系统配置换成一个命名管道（FIFO），核心读配置时卡在打开它上面，进程活着、什么都不说；核心起来以后
//! `SIGSTOP` 它，套接字照旧连得上、握手没人回。等多久走测试口子（`MIYU_TEST_WAIT_SLOW_MS`、`MIYU_TEST_WAIT_LIMIT_MS`，只在
//! `testkit` 打开时认），不真等十分钟。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::support::Home;
use miyu_ipc::ConnectError;

/// 过了多久先说一声（测试口子）。
const SLOW_MS: u64 = 500;

/// 最多等多久（测试口子）。
const LIMIT_MS: u64 = 3000;

/// 跑着的一个头：标准错误一行行收着，带上是第几毫秒到的。
pub struct Watched {
    child: Child,
    started: Instant,
    lines: Arc<Mutex<Vec<(u128, String)>>>,
}

impl Watched {
    /// 照测试口子跑 `miyu <args>`：数据根是 `home` 的，界面语言中文。
    pub fn start(home: &Home, args: &[&str]) -> Watched {
        let root = home.root.path();
        let mut command = crate::support::cli::command(root, root, "zh_CN.UTF-8", args);
        command
            .env("MIYU_TEST_WAIT_SLOW_MS", SLOW_MS.to_string())
            .env("MIYU_TEST_WAIT_LIMIT_MS", LIMIT_MS.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        Watched::spawn(command)
    }

    /// 跑 `command`，收它的标准错误。
    pub fn spawn(mut command: Command) -> Watched {
        let started = Instant::now();
        let mut child = command.spawn().expect("跑得起来");
        let lines = Arc::new(Mutex::new(Vec::new()));
        let stderr = child.stderr.take().expect("接了管道");
        let kept = Arc::clone(&lines);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let at = started.elapsed().as_millis();
                kept.lock().expect("没 panic").push((at, line));
            }
        });
        Watched {
            child,
            started,
            lines,
        }
    }

    /// 等它退，最多 `wait`：交回退出码和退的时刻（毫秒）；到时还没退的杀掉、交回空的。
    pub fn exit_within(&mut self, wait: Duration) -> Option<(ExitStatus, u128)> {
        let until = Instant::now() + wait;
        loop {
            if let Some(status) = self.child.try_wait().expect("问得了") {
                return Some((status, self.started.elapsed().as_millis()));
            }
            if Instant::now() >= until {
                self.child.kill().expect("杀得了");
                self.child.wait().expect("收得了");
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 标准错误里带 `words` 的第一行是第几毫秒到的。
    pub fn told(&self, words: &str) -> Option<u128> {
        let lines = self.lines.lock().expect("没 panic");
        lines
            .iter()
            .find(|(_, line)| line.contains(words))
            .map(|(at, _)| *at)
    }

    /// 标准错误的全文。
    pub fn text(&self) -> String {
        let lines = self.lines.lock().expect("没 panic");
        lines
            .iter()
            .map(|(at, line)| format!("{at}ms {line}\n"))
            .collect()
    }

    /// 进程号。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

/// 系统配置换成一个命名管道：核心读配置时卡住。交回它的位置。
pub fn stuck_config(home: &Home) -> PathBuf {
    let fifo = home.root.path().join("etc/config.toml");
    std::fs::create_dir_all(fifo.parent().expect("有上一级")).expect("建得了");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("跑得了");
    assert!(made.success(), "建得了命名管道");
    fifo
}

/// 往命名管道里喂一次空的：卡在打开它的那一个读到空的配置，接着走。
pub fn feed(fifo: &Path) {
    let fifo = fifo.to_path_buf();
    std::thread::spawn(move || drop(std::fs::OpenOptions::new().write(true).open(fifo)));
}

/// 等运行日志里有进程号（核心装好日志以后、读配置以前就记了）。
pub fn core_pid(home: &Home) -> String {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(pid) = home.core_pid() {
            return pid;
        }
        assert!(Instant::now() < until, "核心没起来：{}", home.core_log());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 核心的套接字可能在的两处（测试的核心不用 `$XDG_RUNTIME_DIR`）：数据根的 `run/core.sock`，路径太长的放临时目录下
/// `miyu-<uid>/<指纹>.sock`（macOS 的临时目录长，常是这一种）。
pub fn sockets(home: &Home) -> [PathBuf; 2] {
    let uid = Command::new("id").arg("-u").output().expect("跑得了");
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    let print = miyu_ipc::fingerprint(&home.root);
    [
        home.root.run().join("core.sock"),
        std::env::temp_dir()
            .join(format!("miyu-{uid}"))
            .join(format!("{print}.sock")),
    ]
}

/// 给进程 `pid` 发信号 `signal`（`STOP`、`CONT`、`KILL`、`TERM`、`INT`）。
pub fn signal(pid: &str, signal: &str) {
    let sent = Command::new("kill")
        .args([format!("-{signal}"), pid.to_string()])
        .status()
        .expect("跑得了");
    assert!(sent.success(), "发得了 {signal} 给 {pid}");
}

/// 核心卡着起不来：过了一会儿先说「还在启动」，到了上限说超时、当场退出，不等核心说话（修之前要等核心写出那一行才退，
/// 2026-11-27 量到 138 秒）。
#[test]
fn a_slow_core_is_told_about_then_given_up_on_in_time() {
    let home = Home::new();
    let fifo = stuck_config(&home);
    let mut head = Watched::start(&home, &["core", "start"]);
    let pid = core_pid(&home);
    let exited = head.exit_within(Duration::from_secs(15));
    let text = head.text();
    let notice = head.told("核心还在启动");
    signal(&pid, "KILL");
    feed(&fifo);
    let (status, at) = exited.unwrap_or_else(|| panic!("十五秒没退：{text}"));
    assert_eq!(status.code(), Some(1), "{text}");
    assert!(head.told("等待核心启动超时").is_some(), "{text}");
    let notice = notice.unwrap_or_else(|| panic!("没说还在启动：{text}"));
    assert!(
        (u128::from(SLOW_MS)..u128::from(SLOW_MS) + 2000).contains(&notice),
        "过了 {SLOW_MS} 毫秒说一声：{text}"
    );
    assert!(
        (u128::from(LIMIT_MS)..u128::from(LIMIT_MS) + 2000).contains(&at),
        "到了上限当场退：{text}"
    );
}

/// 等的时候核心没了：当场说核心没起来，不等到上限。
#[test]
fn a_core_that_dies_while_starting_ends_the_wait_at_once() {
    let home = Home::new();
    let _fifo = stuck_config(&home);
    let mut head = Watched::start(&home, &["core", "start"]);
    let pid = core_pid(&home);
    std::thread::sleep(Duration::from_millis(200));
    let killed = Instant::now();
    signal(&pid, "KILL");
    let exited = head.exit_within(Duration::from_secs(15));
    let text = head.text();
    let (status, _) = exited.unwrap_or_else(|| panic!("十五秒没退：{text}"));
    assert_eq!(status.code(), Some(1), "{text}");
    assert!(head.told("核心没有启动").is_some(), "{text}");
    assert!(
        killed.elapsed() < Duration::from_secs(2),
        "当场结束：{text}"
    );
}

/// 等的时候人按了 Ctrl+C（`SIGINT`）：头当场退。
#[test]
fn ctrl_c_ends_the_wait_at_once() {
    let home = Home::new();
    let _fifo = stuck_config(&home);
    let mut head = Watched::start(&home, &["core", "start"]);
    let pid = core_pid(&home);
    let pressed = Instant::now();
    signal(&head.pid().to_string(), "INT");
    let exited = head.exit_within(Duration::from_secs(15));
    signal(&pid, "KILL");
    assert!(exited.is_some(), "十五秒没退：{}", head.text());
    assert!(pressed.elapsed() < Duration::from_secs(2), "当场退");
}

/// 核心绑好套接字、还没开始接连接：`run/socket` 还不写，头连不上，不会连上了却没人回（修之前第 8 步就写了，连上的头一直
/// 空等）。照命名管道卡在第二次读配置（建好套接字以后），这时连是「没在跑」。
#[tokio::test]
async fn a_core_not_yet_accepting_is_not_reachable() {
    let home = Home::new();
    let fifo = stuck_config(&home);
    let mut core = home
        .core_staying()
        .stdout(Stdio::null())
        .spawn()
        .expect("起得来");
    let pid = core_pid(&home);
    // 第一次读配置（建运行时以前数工作线程）放过去，卡在第二次（建好套接字以后读整份配置）。
    feed(&fifo);
    let sockets = sockets(&home);
    let until = Instant::now() + Duration::from_secs(60);
    while !sockets.iter().any(|socket| socket.exists()) {
        assert!(Instant::now() < until, "没建套接字：{}", home.core_log());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let reached = tokio::time::timeout(Duration::from_secs(5), miyu_ipc::connect(&home.root)).await;
    signal(&pid, "KILL");
    core.wait().expect("收得了");
    feed(&fifo);
    assert!(
        matches!(reached, Ok(Err(ConnectError::NotRunning))),
        "还没开始接连接的当没在跑：{:?}",
        reached.map(|reached| reached.map(|_| ()))
    );
}

/// 核心起来以后卡住（`SIGSTOP`）：连得上，握手没人回。过了一会儿先说「还没有回应」，到了上限说没回应、当场退（修之前一直空等）。
#[test]
fn a_handshake_nobody_answers_is_told_about_then_given_up_on() {
    let home = Home::new();
    let mut started = Watched::start(&home, &["core", "start"]);
    let up = started.exit_within(Duration::from_secs(60));
    assert!(
        up.is_some_and(|(status, _)| status.success()),
        "{}",
        started.text()
    );
    let pid = core_pid(&home);
    signal(&pid, "STOP");
    let mut head = Watched::start(&home, &["config", "get", "ui.language"]);
    let exited = head.exit_within(Duration::from_secs(15));
    signal(&pid, "CONT");
    let text = head.text();
    let (status, at) = exited.unwrap_or_else(|| panic!("十五秒没退：{text}"));
    assert_eq!(status.code(), Some(1), "{text}");
    let notice = head
        .told("核心还没有回应")
        .unwrap_or_else(|| panic!("没说还没有回应：{text}"));
    assert!(notice >= u128::from(SLOW_MS), "{text}");
    assert!(head.told("核心一直没有回应").is_some(), "{text}");
    assert!(
        (u128::from(LIMIT_MS)..u128::from(LIMIT_MS) + 2000).contains(&at),
        "到了上限当场退：{text}"
    );
}
