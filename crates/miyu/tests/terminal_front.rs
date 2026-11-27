//! 人在终端前台直接跑 `miyu core`（施工 CS-11）：一个真的伪终端，`python3` 开新会话、`TIOCSCTTY` 拿它当控制终端，换成一个
//! 交互式的 bash；测试像人一样往里敲 `miyu core`。这个进程有控制终端，只当前台壳（`miyu_core::front`），把真正的核心拉起到新
//! 会话里，挂着终端等它。
//!
//! - 前台一直是壳那一组（测试从主端每 20 毫秒看一次），照 `support/grab.rs` 跑的那几条会去抢终端的命令都跑完、打不开
//!   `/dev/tty`；核心的会话是它自己开的（命令问出来的会话号是核心的进程号），不是壳。按 Ctrl+C（只到前台组，也就是壳），壳
//!   转给核心，核心有计划地停下，壳照核心的退出码 0 退出。
//! - 关掉终端（主端关了）：bash 收到 `SIGHUP`、转给前台的作业，壳再转给核心，核心退出。
//! - 核心被 `SIGKILL`：壳照 shell 的写法交 137（128 加 9）。

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::support::pty::{Capture, pty};
use crate::support::{Home, MIYU, count, grab, within};

/// 开新会话，把伪终端 `argv[1]` 设成控制终端、标准输入输出错误都接它，换成 `argv[2:]`。别的描述符都关掉：测试的主端开着时没带
/// `O_CLOEXEC`，漏进 bash、核心的话测试关了主端终端也不算关。
const LEAD: &str = r#"
import fcntl, os, sys, termios
os.setsid()
fd = os.open(sys.argv[1], os.O_RDWR | os.O_NOCTTY)
fcntl.ioctl(fd, termios.TIOCSCTTY, 0)
for n in (0, 1, 2):
    os.dup2(fd, n)
os.closerange(3, 65536)
os.execvp(sys.argv[2], sys.argv[2:])
"#;

/// 交互式的 bash：不读谁的配置、不写历史，提示符是 `$ `。
const BASH: &[&str] = &["bash", "--norc", "--noprofile", "-i"];

/// 拿着终端的那个进程（bash，或者换成了的核心）：丢掉时杀掉、收掉。
struct Leader(Child);

impl Drop for Leader {
    #[expect(clippy::let_underscore_must_use, reason = "收尾：已经退了的杀不了")]
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 在伪终端 `tty` 上起 `program`，让它拿着这个终端（[`LEAD`]）。环境同测试里跑的 `miyu`（数据根、资源、没有 key），`HOME`
/// 指向工作目录里的空目录。
fn lead(home: &Home, work: &Path, tty: &str, program: &[&str]) -> Leader {
    let inner = crate::support::cli::command(home.root.path(), work, "zh_CN.UTF-8", &[]);
    let quiet = work.join("home");
    std::fs::create_dir_all(&quiet).expect("建得了");
    let mut command = Command::new("python3");
    command
        .args(["-c", LEAD, tty])
        .args(program)
        .current_dir(work);
    for (key, value) in inner.get_envs() {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    command
        .env("HOME", &quiet)
        .env("HISTFILE", "/dev/null")
        .env("PS1", "$ ")
        .env("BASH_SILENCE_DEPRECATION_WARNING", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Leader(command.spawn().expect("起得来 python3"))
}

/// 敲 `miyu core`、回车。
fn type_core(main: &File) {
    let line = format!("'{MIYU}' core\n");
    (&*main).write_all(line.as_bytes()).expect("写得进");
}

/// 等核心起来（连得上），交回它的进程号。
async fn up(home: &Home) -> String {
    within("核心起来", async {
        while miyu_ipc::connect(&home.root).await.is_err() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    home.core_pid().expect("运行日志里有进程号")
}

/// 终端这一刻的前台进程组（从主端问）；问不出的是 -1。
fn front(main: &File) -> i32 {
    rustix::termios::tcgetpgrp(main).map_or(-1, |pid| rustix::process::Pid::as_raw(Some(pid)))
}

/// 每 20 毫秒记一次前台进程组，直到 `stop`。
fn watch(main: File, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<BTreeSet<i32>> {
    std::thread::spawn(move || {
        let mut seen = BTreeSet::new();
        while !stop.load(Ordering::SeqCst) {
            seen.insert(front(&main));
            std::thread::sleep(Duration::from_millis(20));
        }
        seen.insert(front(&main));
        seen
    })
}

#[tokio::test]
async fn a_core_run_in_the_foreground_lets_go_of_the_terminal_and_still_hears_ctrl_c() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作目录");
    let (main, tty) = pty();
    let screen = Capture::start(&main);
    let bash = lead(&home, &work, &tty, BASH);
    screen.wait_for("$ ");
    type_core(&main);
    let core = up(&home).await;
    let first = front(&main);
    assert_ne!(
        first.to_string(),
        core,
        "前台是壳，不是核心：{}",
        screen.text()
    );
    assert_ne!(
        first,
        i32::try_from(bash.0.id()).expect("放得下"),
        "壳是前台的作业"
    );
    let stop = Arc::new(AtomicBool::new(false));
    let seen = watch(main.try_clone().expect("克隆得出"), Arc::clone(&stop));

    let ran = grab::run(&home, &work, "cs11").await;
    stop.store(true, Ordering::SeqCst);
    let seen = seen.join().expect("没 panic");
    let log = home.core_log();
    ran.check(&format!("前台：{seen:?}；核心的日志：{log}"));
    assert_eq!(seen, BTreeSet::from([first]), "终端的前台被换掉了");
    assert_eq!(ran.output("sid.out").trim(), core, "核心自己开了一个会话");

    (&main).write_all(b"\x03").expect("写得进");
    home.until_stopped().await;
    assert_eq!(
        count(&home.core_log(), "stopped reason=signal"),
        1,
        "Ctrl+C 停下的：{}",
        home.core_log()
    );
    (&main).write_all(b"echo \"code=$?\"\n").expect("写得进");
    screen.wait_for("code=0");
}

#[tokio::test]
async fn closing_the_terminal_stops_a_core_run_in_the_foreground() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作目录");
    let (main, tty) = pty();
    let _bash = lead(&home, &work, &tty, BASH);
    // 等提示符：只用这一份主端读，不另外克隆，关的时候终端才真的关掉。
    let main = tokio::task::spawn_blocking(move || {
        let mut seen = Vec::new();
        let mut chunk = [0u8; 256];
        while !String::from_utf8_lossy(&seen).contains("$ ") {
            let n = (&main).read(&mut chunk).expect("读得到");
            assert_ne!(n, 0, "bash 没起来");
            seen.extend_from_slice(&chunk[..n]);
        }
        main
    })
    .await
    .expect("没 panic");
    type_core(&main);
    up(&home).await;

    drop(main);
    home.until_stopped().await;
}

#[tokio::test]
async fn the_shell_exits_with_the_core_s_code() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作目录");
    let (main, tty) = pty();
    let screen = Capture::start(&main);
    let _bash = lead(&home, &work, &tty, BASH);
    screen.wait_for("$ ");
    type_core(&main);
    let core = up(&home).await;
    let killed = Command::new("kill").args(["-KILL", &core]).status();
    assert!(killed.is_ok_and(|status| status.success()), "杀得了 {core}");
    home.until_stopped().await;
    (&main).write_all(b"echo \"code=$?\"\n").expect("写得进");
    // 回车以后 bash 先写关掉括号粘贴的控制序列，同一行：照子串找。印出来的不是 137 的（比如 1）也先停下来、断言时报。
    screen.wait_for("code=1");
    let said = screen.text();
    assert!(said.contains("code=137"), "{said}");
}
