//! 会去抢终端的几条命令（施工 CS-10 写在 `terminal.rs` 里，CS-11 挪出来给 `terminal_front.rs` 共用）：连上核心，人用 `!` 跑
//! （和她调 `shell` 走同一个起法，`basesystem` 的 `process::spawn`），每条一个会话、并着跑，输出写进工作目录的 `<名>.out`，
//! 退出码写进 `<名>.code`。
//!
//! - 一个 `python3` 小程序：打开 `/dev/tty`、`tcsetpgrp` 把前台抢给自己就退（CI 上没有 zsh 也能测「抢了不还」）；
//! - `sh -c 'exec </dev/tty'`：只打开终端；
//! - 有 bash 的：`bash --norc -i`、`bash -lic 'echo hi'`（Linux 上的 bash 5 打得开终端又不在前台时，`SIGTTIN` 把自己停住、
//!   一直挂着；macOS 自带的 bash 3.2 不停住，LiteMiyu TF-1 在 M1 上记的）；
//! - 有 zsh 的：`zsh -i`（抢走前台、一直读终端）、`zsh -i -c 'echo hi'`、`zsh -lic 'echo hi'`（抢走前台马上退，前台留给不在了
//!   的组；`-lic` 是 LiteMiyu TF-1 撞到的写法）；
//! - 另有一条问核心的会话号（命令照继承在核心的会话里）。
//!
//! 要的是（[`Ran::check`]）：几条都在时限内跑完，打不开 `/dev/tty`，带 `-c 'echo hi'` 的照常印出 hi。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

use super::{Home, within};
use crate::crash::Rpc;

/// 每条命令最多跑多久（几条并着跑，共用这一个时限）：抢得到终端的挂住的几条到这里还没完。
pub const LIMIT: Duration = Duration::from_secs(30);

/// 抢终端的小程序：打开 `/dev/tty`，不理 `SIGTTOU`（zsh 也这样），把前台抢给自己就退。没有控制终端的打不开，退出码 1。
const STEAL: &str = r#"
import os, signal
signal.signal(signal.SIGTTOU, signal.SIG_IGN)
fd = os.open("/dev/tty", os.O_RDWR)
os.tcsetpgrp(fd, os.getpgrp())
print("stole")
"#;

/// 一条要跑的命令：起名、命令本身。
pub struct Probe {
    pub name: &'static str,
    pub command: String,
}

/// 跑完（或者到时）的几条。
pub struct Ran {
    pub probes: Vec<Probe>,
    /// 到时还没跑完的。
    pub hung: Vec<&'static str>,
    work: PathBuf,
}

impl Ran {
    /// 工作目录里的文件 `name`：没有的是空的。
    pub fn output(&self, name: &str) -> String {
        std::fs::read_to_string(self.work.join(name)).unwrap_or_default()
    }

    /// 几条都跑完了，打不开 `/dev/tty`，带 `-c 'echo hi'` 的印出 hi。`context` 接在没跑完的那一句后面（头记下的、核心的日志）。
    pub fn check(&self, context: &str) {
        assert!(
            self.hung.is_empty(),
            "这几条没在 {LIMIT:?} 里跑完：{:?}；{context}",
            self.hung
        );
        for name in ["steal", "tty"] {
            assert_ne!(
                self.output(&format!("{name}.code")).trim(),
                "0",
                "{name} 打开了 /dev/tty：{}",
                self.output(&format!("{name}.out"))
            );
        }
        for probe in self
            .probes
            .iter()
            .filter(|probe| probe.command.ends_with("'echo hi'"))
        {
            let out = self.output(&format!("{}.out", probe.name));
            assert!(
                out.lines().any(|line| line == "hi"),
                "{} 没印出 hi：{out}",
                probe.command
            );
        }
    }
}

/// 这台机器上有没有 `program`（照 `PATH` 找）。
fn has(program: &str) -> bool {
    Command::new("/bin/sh")
        .args(["-c", &format!("command -v {program}")])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// 要跑的几条：没有 bash、zsh 的跳过那几条。zsh、bash 不读开发者自己的配置（`ZDOTDIR`、`HOME` 指向一个空目录）。
fn probes(work: &Path) -> Vec<Probe> {
    std::fs::write(work.join("steal.py"), STEAL).expect("写得进");
    let quiet = work.join("zdot");
    std::fs::create_dir_all(&quiet).expect("建得了");
    let zsh = format!("ZDOTDIR='{}' zsh", quiet.display());
    let probe = |name, command: &str| Probe {
        name,
        command: command.to_string(),
    };
    let mut probes = vec![
        probe("sid", "python3 -c 'import os; print(os.getsid(0))'"),
        probe("steal", "python3 steal.py"),
        probe("tty", "sh -c 'exec </dev/tty'"),
    ];
    if has("bash") {
        probes.push(probe("bash", "bash --norc -i"));
        let lic = format!("HOME='{}' bash -lic 'echo hi'", quiet.display());
        probes.push(probe("bash_lic", &lic));
    }
    if has("zsh") {
        probes.push(probe("zsh", &format!("{zsh} -i")));
        probes.push(probe("zsh_c", &format!("{zsh} -i -c 'echo hi'")));
        probes.push(probe("zsh_lic", &format!("{zsh} -lic 'echo hi'")));
    }
    probes
}

/// 等文件 `path` 出现，最多等 `limit`。
pub async fn appears(path: &Path, limit: Duration) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if path.exists() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    path.exists()
}

/// 连上 `home` 的核心，在工作目录 `work` 里把几条并着跑，等到都跑完或者到时（[`LIMIT`]）。RPC 的编号带上 `tag`。
pub async fn run(home: &Home, work: &Path, tag: &str) -> Ran {
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let mut rpc = Rpc::over(connection);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call(&format!("{tag}-hello"), "hello", hello).await;
    let probes = probes(work);
    for probe in &probes {
        let created = rpc
            .call(
                &format!("{tag}-create-{}", probe.name),
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
            &format!("{tag}-send-{}", probe.name),
            "session.send",
            json!({"session": session, "text": text, "shell": true}),
        )
        .await;
    }
    let until = Instant::now() + LIMIT;
    let mut hung = Vec::new();
    for probe in &probes {
        let left = until.saturating_duration_since(Instant::now());
        if !appears(&work.join(format!("{}.code", probe.name)), left).await {
            hung.push(probe.name);
        }
    }
    Ran {
        probes,
        hung,
        work: work.to_path_buf(),
    }
}
