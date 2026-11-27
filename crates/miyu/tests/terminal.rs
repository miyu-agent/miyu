//! 交互式命令不许抢走终端（施工 CS-10）：一个真的伪终端，一个拿着它当控制终端的头（`python3`：开新会话、`TIOCSCTTY`，
//! 再像人一样在里面敲 `miyu core start`），核心由它照真的拉起法（`miyu_ipc` 的 `detach`）拉起。核心起来以后，人用 `!` 跑几条
//! 会去抢终端的命令（和她调 `shell` 走同一个起法，`basesystem` 的 `process::spawn`），每条一个会话、并着跑：
//!
//! 跑的几条和要的样子见 `support/grab.rs`（CS-11 挪过去和 `terminal_front.rs` 共用）。头每 20 毫秒记一次终端的前台进程组。另外要的
//! 是：前台一直是头自己的组；核心的会话是它自己开的（会话号是它的进程号），不是头的，也没有控制终端。

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::support::grab::{self, appears};
use crate::support::pty::{Capture, pty};
use crate::support::{Home, MIYU};

/// 头：开新会话，把伪终端 `argv[1]` 设成控制终端（自己的组就是前台），在里面跑 `argv[4:]`（`miyu core start`）。跑完写
/// `argv[2].started`（自己的会话、组、退出码），之后每 20 毫秒记一次前台进程组，直到 `argv[3]` 出现，记下的写进 `argv[2]`。
const HEAD: &str = r#"
import fcntl, json, os, subprocess, sys, termios, time
tty, out, done, cmd = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
os.setsid()
fd = os.open(tty, os.O_RDWR | os.O_NOCTTY)
fcntl.ioctl(fd, termios.TIOCSCTTY, 0)
os.dup2(fd, 0)
os.dup2(fd, 1)
code = subprocess.run(cmd).returncode
info = {"sid": os.getsid(0), "pgrp": os.getpgrp(), "code": code}
def put(path, value):
    with open(path + ".part", "w") as f:
        json.dump(value, f)
    os.rename(path + ".part", path)
put(out + ".started", info)
seen = set()
while not os.path.exists(done):
    seen.add(os.tcgetpgrp(fd))
    time.sleep(0.02)
seen.add(os.tcgetpgrp(fd))
info["foreground"] = sorted(seen)
put(out, info)
"#;

/// 起着的头：丢掉时叫它停（写 `done`），不停的杀掉。
struct Head {
    child: Child,
    done: PathBuf,
}

impl Drop for Head {
    #[expect(clippy::let_underscore_must_use, reason = "收尾：停不了的照样往下")]
    fn drop(&mut self) {
        let _ = std::fs::write(&self.done, "");
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until && matches!(self.child.try_wait(), Ok(None)) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 读头写的那一份。
fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("头写了")).expect("是 JSON")
}

/// 进程 `pid` 的控制终端，照 `ps -o tty=`：没有的 Linux 上印 `?`，macOS 上印 `??`。
fn tty_of(pid: &str) -> String {
    let out = Command::new("ps")
        .args(["-o", "tty=", "-p", pid])
        .output()
        .expect("跑得了 ps");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[tokio::test]
async fn interactive_commands_do_not_take_the_terminal_of_the_head_that_started_the_core() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作目录");
    let (main, name) = pty();
    // 一直读着主端：修之前抢到终端的 zsh 往里写提示符，没人读写满了会卡住它。
    let _screen = Capture::start(&main);
    let out = home.dir.join("head.json");
    let done = home.dir.join("head.done");
    let inner =
        crate::support::cli::command(home.root.path(), &work, "zh_CN.UTF-8", &["core", "start"]);
    let mut command = Command::new("python3");
    command
        .args(["-c", HEAD, &name])
        .arg(&out)
        .arg(&done)
        .arg(MIYU)
        .args(inner.get_args())
        .current_dir(&work);
    for (key, value) in inner.get_envs() {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let errors = home.dir.join("head.err");
    let log = std::fs::File::create(&errors).expect("建得了");
    let head = Head {
        child: command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .expect("起得来 python3"),
        done: done.clone(),
    };
    let started = out.with_extension("json.started");
    assert!(
        appears(&started, Duration::from_secs(60)).await,
        "头没拉起核心：{}",
        std::fs::read_to_string(&errors).unwrap_or_default()
    );
    let info = read(&started);
    assert_eq!(info["code"], json!(0), "miyu core start：{info}");
    let core = home.core_pid().expect("核心起来了");

    let ran = grab::run(&home, &work, "cs10").await;
    let core_tty = tty_of(&core);

    drop(head);
    let after = read(&out);
    ran.check(&format!(
        "头记下的：{after}；核心的日志：{}",
        home.core_log()
    ));
    assert_eq!(
        after["foreground"],
        json!([after["pgrp"]]),
        "终端的前台被换掉了"
    );
    let core_sid = ran.output("sid.out").trim().to_string();
    assert_ne!(core_sid, after["sid"].to_string(), "核心还在头的会话里");
    assert_eq!(core_sid, core, "核心自己开了一个会话");
    assert!(
        !core_tty.is_empty() && core_tty.chars().all(|c| c == '?'),
        "核心还有控制终端：{core_tty}"
    );
    home.kill_core().await;
}
