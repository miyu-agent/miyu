//! 交互式命令不许抢走终端（施工 CS-10）：一个真的伪终端，一个拿着它当控制终端的头（`python3`：开新会话、`TIOCSCTTY`，
//! 再像人一样在里面敲 `miyu core start`），核心由它照真的拉起法（`miyu_ipc` 的 `detach`）拉起。核心起来以后，人用 `!` 跑几条
//! 会去抢终端的命令（和她调 `shell` 走同一个起法，`basesystem` 的 `process::spawn`），每条一个会话、并着跑：
//!
//! - 一个 `python3` 小程序：打开 `/dev/tty`、`tcsetpgrp` 把前台抢给自己就退（CI 上没有 zsh 也能测「抢了不还」）；
//! - `sh -c 'exec </dev/tty'`：只打开终端；
//! - 有 bash 的：`bash --norc -i`、`bash -lic 'echo hi'`（修之前 Linux 上的 bash 5 发现自己不在前台，`SIGTTIN` 把自己停住，
//!   一直挂着；macOS 自带的 bash 3.2 不停住，LiteMiyu TF-1 在 M1 上记的）；
//! - 有 zsh 的：`zsh -i`（修之前抢走前台、一直读终端）、`zsh -i -c 'echo hi'`、`zsh -lic 'echo hi'`（修之前抢走前台马上退，
//!   前台留给不在了的组；`-lic` 是 LiteMiyu TF-1 撞到的写法）。
//!
//! 另有一条问核心的会话号（命令照继承在核心的会话里）。头每 20 毫秒记一次终端的前台进程组。要的是：前台一直是头自己的组；
//! 核心的会话是它自己开的（会话号是它的进程号），不是头的，也没有控制终端；几条命令都在时限内跑完，打不开 `/dev/tty`，
//! 带 `-c 'echo hi'` 的照常印出 hi。

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::crash::Rpc;
use crate::support::pty::{Capture, pty};
use crate::support::{Home, MIYU, within};

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

/// 抢终端的小程序：打开 `/dev/tty`，不理 `SIGTTOU`（zsh 也这样），把前台抢给自己就退。没有控制终端的打不开，退出码 1。
const STEAL: &str = r#"
import os, signal
signal.signal(signal.SIGTTOU, signal.SIG_IGN)
fd = os.open("/dev/tty", os.O_RDWR)
os.tcsetpgrp(fd, os.getpgrp())
print("stole")
"#;

/// 每条命令最多跑多久：修之前挂住的几条到这里还没完。
const LIMIT: Duration = Duration::from_secs(30);

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

/// 一条要跑的命令：起名、命令本身。输出写进 `<名>.out`，退出码写进 `<名>.code`。
struct Probe {
    name: &'static str,
    command: String,
}

/// 这台机器上有没有 `program`（照 `PATH` 找）。
fn has(program: &str) -> bool {
    Command::new("/bin/sh")
        .args(["-c", &format!("command -v {program}")])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// 要跑的几条：没有 bash、zsh 的跳过那几条。zsh 不读开发者自己的配置（`ZDOTDIR` 指向一个空目录）。
fn probes(work: &Path) -> Vec<Probe> {
    std::fs::write(work.join("steal.py"), STEAL).expect("写得进");
    let quiet = work.join("zdot");
    std::fs::create_dir_all(&quiet).expect("建得了");
    let zsh = format!("ZDOTDIR='{}' zsh", quiet.display());
    let mut probes = vec![
        // 核心的会话号：命令照继承在核心的会话里，问自己的就是。
        Probe {
            name: "sid",
            command: "python3 -c 'import os; print(os.getsid(0))'".into(),
        },
        Probe {
            name: "steal",
            command: "python3 steal.py".into(),
        },
        Probe {
            name: "tty",
            command: "sh -c 'exec </dev/tty'".into(),
        },
    ];
    if has("bash") {
        probes.push(Probe {
            name: "bash",
            command: "bash --norc -i".into(),
        });
        // 登录加交互（LiteMiyu TF-1 撞到的写法）：不读开发者自己的配置（`HOME` 指向空目录）。
        probes.push(Probe {
            name: "bash_lic",
            command: format!("HOME='{}' bash -lic 'echo hi'", quiet.display()),
        });
    }
    if has("zsh") {
        probes.push(Probe {
            name: "zsh",
            command: format!("{zsh} -i"),
        });
        probes.push(Probe {
            name: "zsh_c",
            command: format!("{zsh} -i -c 'echo hi'"),
        });
        probes.push(Probe {
            name: "zsh_lic",
            command: format!("{zsh} -lic 'echo hi'"),
        });
    }
    probes
}

/// 等文件 `path` 出现，最多等 `limit`。
async fn appears(path: &Path, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if path.exists() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    path.exists()
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

    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let mut rpc = Rpc::over(connection);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call("cs10-hello", "hello", hello).await;
    let probes = probes(&work);
    for probe in &probes {
        let created = rpc
            .call(
                &format!("cs10-create-{}", probe.name),
                "session.create",
                json!({"cwd": work.to_string_lossy()}),
            )
            .await;
        let session = created["session"].as_str().expect("有编号").to_string();
        // 输出照绝对路径写：人跑的命令不一定在会话的工作目录里跑。
        let text = format!(
            "cd '{dir}' && {} > {name}.out 2>&1; echo $? > '{dir}/{name}.code'",
            probe.command,
            dir = work.display(),
            name = probe.name
        );
        rpc.call(
            &format!("cs10-send-{}", probe.name),
            "session.send",
            json!({"session": session, "text": text, "shell": true}),
        )
        .await;
    }
    // 几条并着跑，共用一个时限。
    let until = Instant::now() + LIMIT;
    let mut hung = Vec::new();
    for probe in &probes {
        let left = until.saturating_duration_since(Instant::now());
        if !appears(&work.join(format!("{}.code", probe.name)), left).await {
            hung.push(probe.name);
        }
    }
    let output = |name: &str| std::fs::read_to_string(work.join(name)).unwrap_or_default();
    let core_tty = tty_of(&core);

    drop(head);
    let after = read(&out);
    assert!(
        hung.is_empty(),
        "这几条没在 {LIMIT:?} 里跑完：{hung:?}；头记下的：{after}；核心的日志：{}",
        home.core_log()
    );
    assert_eq!(
        after["foreground"],
        json!([after["pgrp"]]),
        "终端的前台被换掉了"
    );
    let core_sid = output("sid.out").trim().to_string();
    assert_ne!(core_sid, after["sid"].to_string(), "核心还在头的会话里");
    assert_eq!(core_sid, core, "核心自己开了一个会话");
    assert!(
        !core_tty.is_empty() && core_tty.chars().all(|c| c == '?'),
        "核心还有控制终端：{core_tty}"
    );
    for name in ["steal", "tty"] {
        assert_ne!(
            output(&format!("{name}.code")).trim(),
            "0",
            "{name} 打开了 /dev/tty：{}",
            output(&format!("{name}.out"))
        );
    }
    for probe in probes
        .iter()
        .filter(|probe| probe.command.ends_with("'echo hi'"))
    {
        let out = output(&format!("{}.out", probe.name));
        assert!(
            out.lines().any(|line| line == "hi"),
            "{} 没印出 hi：{out}",
            probe.command
        );
    }
    home.kill_core().await;
}
