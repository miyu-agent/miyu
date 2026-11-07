//! 核心崩了，命令跟着没（施工 7-8，`docs/blueprint/core.md`「子进程随核心退出」；Unix 上前台、监视的、人跑的几行和看门的是谁
//! 是施工 V-48 补的）：真的 `miyu core`，模型是本机回环上的假服务器（照 DeepSeek 的写法回一次调 `shell`、再说一句）。命令是
//! 一个心跳：一个子 shell 在后台每 0.1 秒往文件里写一行（Unix 上它是命令起的孙进程，命令自己和它的组也记下；Windows 上是
//! PowerShell 自己）。心跳跑起来以后硬杀核心（Unix 上 `SIGKILL`，Windows 上结束进程，不连子进程），心跳要在几秒内停下：Unix
//! 上组里看门的杀了整组，Windows 上核心所在的作业对象跟着关。
//!
//! - 她放到后台的（三个平台）；Unix 上再测她在前台跑着的、她监视的（MN-1 上）、人用 `!` 跑的（SH-1 上，不经模型）：命令自己、
//!   孙进程都要没了；主程序旁边有助手的，组里看门的是助手（施工 V-48），没有的是 `/bin/sh`。
//! - 两个核心各有各的看门的：硬杀一个只停它的；另一个有计划地停下（SIGTERM），命令停、看门的也没了。
//!
//! 没停下的（修之前的样子），测试最后照心跳记下的进程号把它杀掉：只杀测试自己起的。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use crate::support::{Home, within};
use miyu_http::testkit::{Piece, Reply, Server};

/// 一段事件流：照 DeepSeek 的写法，一块 `data:` 一行，最后 `[DONE]`。
fn stream(chunks: &[Value]) -> Reply {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 模型的回复：先照 `args` 调 `shell`，再说一句。人用 `!` 跑的命令（[`How::Person`]，`args` 是空的）不经模型，只备着她
/// 回的那一句。
fn replies(args: Option<Value>) -> Vec<Reply> {
    let call = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "tool_calls": [{
        "index": 0, "id": "call_a", "type": "function",
        "function": {"name": "shell", "arguments": args.as_ref().map(Value::to_string)}}]}, "finish_reason": null}]});
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]});
    let said = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": "放出去了。"},
        "finish_reason": "stop"}]});
    match args {
        Some(_) => vec![stream(&[call, done]), stream(&[said])],
        None => vec![stream(&[said])],
    }
}

/// 命令怎么起的（施工 V-48 的那张表）。
#[derive(Debug, Clone, Copy)]
enum How {
    /// 她放到后台的（施工 7-3）。
    Background,
    /// 她在前台跑、还没跑完的。
    #[cfg(unix)]
    Foreground,
    /// 她监视的后台命令（施工 MN-1 上）。
    #[cfg(unix)]
    Watched,
    /// 人用 `!` 跑的（施工 SH-1 上）：不经模型、不进沙盒。
    #[cfg(unix)]
    Person,
}

impl How {
    /// 模型调 `shell` 的参数；人跑的是空的。
    fn args(self, command: &str) -> Option<Value> {
        let mut args = json!({"command": command, "description": "心跳"});
        match self {
            How::Background => args["run_in_background"] = json!(true),
            #[cfg(unix)]
            How::Foreground => {}
            #[cfg(unix)]
            How::Watched => args["watch"] = json!(true),
            #[cfg(unix)]
            How::Person => return None,
        }
        Some(args)
    }
}

/// 心跳的几样：`beat` 每 0.1 秒多一行；`pid` 是写心跳的进程（Unix 上是命令起的孙进程）；Unix 上 `command` 是命令自己，
/// `group` 是它的进程组（组号就是看门的进程号）。
struct Beat {
    beat: PathBuf,
    pid: PathBuf,
    command: PathBuf,
    #[cfg(unix)]
    group: PathBuf,
}

impl Beat {
    fn in_dir(work: &Path) -> Beat {
        Beat {
            beat: work.join("beat.txt"),
            pid: work.join("beat.pid"),
            command: work.join("command.pid"),
            #[cfg(unix)]
            group: work.join("group.txt"),
        }
    }
}

/// 心跳：命令记下自己和自己的组，再在后台起一个子 shell 每 0.1 秒写一行，记下它的进程号。
#[cfg(unix)]
fn heartbeat(beat: &Beat) -> String {
    let (file, pid) = (beat.beat.display(), beat.pid.display());
    let (command, group) = (beat.command.display(), beat.group.display());
    format!(
        "echo $$ > '{command}'; ps -o pgid= -p $$ > '{group}'; (while :; do echo x >> '{file}'; sleep 0.1; done) & echo $! > '{pid}'; wait"
    )
}

/// 同上，PowerShell。
#[cfg(windows)]
fn heartbeat(beat: &Beat) -> String {
    let (file, pid) = (beat.beat.display(), beat.pid.display());
    format!(
        "Set-Content -Path '{pid}' -Value $PID; while ($true) {{ Add-Content -Path '{file}' -Value x; Start-Sleep -Milliseconds 100 }}"
    )
}

/// 杀掉心跳记下的那个进程（只在它没跟着核心停下时用）。
fn kill_heartbeat(pid: &Path) {
    let Ok(pid) = std::fs::read_to_string(pid) else {
        return;
    };
    let pid = pid.trim();
    #[cfg(unix)]
    let killed = Command::new("kill").args(["-9", pid]).status();
    #[cfg(windows)]
    let killed = Command::new("taskkill")
        .args(["/F", "/T", "/PID", pid])
        .status();
    #[expect(clippy::let_underscore_must_use, reason = "收拾不掉也只是留下一个心跳")]
    let _ = killed;
}

/// 心跳文件有多大。
fn size(beat: &Path) -> u64 {
    std::fs::metadata(beat).map_or(0, |meta| meta.len())
}

/// 等到心跳一秒不再长，最多等 `limit`；交回停没停。
fn settled(beat: &Path, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    let mut last = size(beat);
    let mut since = Instant::now();
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        let now = size(beat);
        if now != last {
            (last, since) = (now, Instant::now());
        } else if since.elapsed() >= Duration::from_secs(1) {
            return true;
        }
    }
    false
}

/// 在连接上说 JSON-RPC：发一条，读到它的回应为止（推送跳过）。`crash_kept.rs` 也用（施工 XH-1）。
pub(crate) struct Rpc {
    read: AsyncReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Rpc {
    /// 在 `connection` 上说。
    pub(crate) fn over(connection: miyu_ipc::Connection) -> Rpc {
        let (read, write) = tokio::io::split(connection);
        Rpc {
            read: AsyncReader::new(read),
            write,
        }
    }

    pub(crate) async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let mut line = String::new();
            within(&format!("{method} 的回应"), self.read.read_line(&mut line))
                .await
                .expect("读得到");
            let reply: Value = serde_json::from_str(&line).expect("是 JSON");
            if reply["id"] == json!(id) {
                assert!(reply.get("error").is_none(), "{method}：{reply}");
                return reply["result"].clone();
            }
        }
    }
}

/// 拉起核心：模型是 `server`（照 `cargo xtask dev-home` 造的那样写一份系统配置，施工 8-6），等它写来 `ready`。
fn start(home: &Home, server: &Server) -> Child {
    let config = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\ncatalog = \"deepseek\"\nkey = {{ env = \"MIYU_TEST_KEY\" }}\n\n[models]\nchat = \"dev/deepseek-chat\"\n",
        server.base_url
    );
    std::fs::write(home.root.config_file(), config).expect("写得进");
    let mut core = home
        .core()
        .env("MIYU_TEST_KEY", "test")
        .env("NO_PROXY", "127.0.0.1")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

/// 起着心跳的一个核心：核心、连接、假模型（连接、假模型留到核心被杀以后：人跑的那一条连接断了也不该停）。
struct Beating {
    core: Child,
    _rpc: Rpc,
    _server: Server,
}

/// 拉起核心、连上、开一个权限全开的会话（工作目录是心跳所在的目录），照 `how` 起心跳，等它跑起来。
async fn beating(home: &Home, beat: &Beat, how: How) -> Beating {
    let work = beat.beat.parent().expect("在工作目录里").to_path_buf();
    let command = heartbeat(beat);
    let server = Server::start(replies(how.args(&command))).await;
    let core = start(home, &server);
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let mut rpc = Rpc::over(connection);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call("hello-1", "hello", hello).await;
    let created = rpc
        .call(
            "c1",
            "session.create",
            json!({"cwd": work.to_string_lossy()}),
        )
        .await;
    let session = created["session"].as_str().expect("有编号").to_string();
    rpc.call(
        "c2",
        "session.set_permission_level",
        json!({"session": session, "level": "full"}),
    )
    .await;
    let send = match how.args(&command) {
        Some(_) => json!({"session": session, "text": "跑个心跳"}),
        None => json!({"session": session, "text": command, "shell": true}),
    };
    rpc.call("c3", "session.send", send).await;
    // 心跳跑起来了：文件在长。
    let started = Instant::now();
    while size(&beat.beat) < 3 && started.elapsed() < Duration::from_secs(60) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(size(&beat.beat) >= 3, "心跳没跑起来：{}", home.core_log());
    Beating {
        core,
        _rpc: rpc,
        _server: server,
    }
}

/// 等心跳停下（最多 60 秒），没停的照记下的进程号收拾掉；交回停没停。
async fn stopped(beat: &Beat) -> bool {
    let file = beat.beat.clone();
    let stopped = tokio::task::spawn_blocking(move || settled(&file, Duration::from_secs(60)))
        .await
        .expect("没 panic");
    if !stopped {
        kill_heartbeat(&beat.pid);
        kill_heartbeat(&beat.command);
    }
    stopped
}

/// 照 `how` 起心跳，硬杀核心，心跳要停：核心没了，命令跟着没。Unix 上命令自己、写心跳的孙进程都要没了；主程序旁边有助手的，
/// 组里看门的是助手（施工 V-48），没有的是 `/bin/sh`。
async fn dies_with_a_killed_core(how: How) {
    let home = Home::new();
    let work: PathBuf = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let beat = Beat::in_dir(&work);
    let Beating {
        mut core,
        _rpc,
        _server,
    } = beating(&home, &beat, how).await;
    #[cfg(unix)]
    let watcher = watcher(&beat);

    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
    let stopped = stopped(&beat).await;
    assert!(stopped, "核心没了，命令还在跑（{how:?}）");
    #[cfg(unix)]
    {
        for pid in [&beat.pid, &beat.command] {
            let pid = std::fs::read_to_string(pid).expect("记下了");
            assert!(gone(pid.trim()), "{how:?}：{pid} 还在");
        }
        let helper = Path::new(crate::support::MIYU).with_file_name("miyu-sandbox");
        let expected = if helper.is_file() {
            "miyu-sandbox"
        } else {
            "sh"
        };
        assert!(
            watcher.ends_with(expected),
            "{how:?}：看门的是 {watcher}，该是 {expected}"
        );
    }
}

/// 组里看门的叫什么：组号就是它的进程号。
#[cfg(unix)]
fn watcher(beat: &Beat) -> String {
    let group = std::fs::read_to_string(&beat.group).expect("记下了组");
    let out = Command::new("ps")
        .args(["-o", "comm=", "-p", group.trim()])
        .output()
        .expect("问得了");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// 进程 `pid` 没了（最多等 5 秒）：`kill -0` 问不到。
#[cfg(unix)]
fn gone(pid: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let there = Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !there {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
async fn a_background_command_dies_with_a_killed_core() {
    dies_with_a_killed_core(How::Background).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_foreground_command_dies_with_a_killed_core() {
    dies_with_a_killed_core(How::Foreground).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_watched_command_dies_with_a_killed_core() {
    dies_with_a_killed_core(How::Watched).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_person_command_dies_with_a_killed_core() {
    dies_with_a_killed_core(How::Person).await;
}

/// 两个核心各有各的看门的：硬杀一个，只有它的命令停；另一个照常跑，有计划地停下（SIGTERM）以后命令停、看门的也没了，不留孤儿。
#[cfg(unix)]
#[tokio::test]
async fn two_cores_watch_their_own_commands_and_a_stopped_core_leaves_no_watcher() {
    let (one, two) = (Home::new(), Home::new());
    let mut beats = Vec::new();
    for home in [&one, &two] {
        let work = home.dir.join("work");
        std::fs::create_dir_all(&work).expect("建得了");
        beats.push(Beat::in_dir(&work));
    }
    let mut first = beating(&one, &beats[0], How::Background).await;
    let second = beating(&two, &beats[1], How::Background).await;
    let group = std::fs::read_to_string(&beats[1].group).expect("记下了组");

    first.core.kill().expect("杀得了核心");
    first.core.wait().expect("收得了");
    let one_stopped = stopped(&beats[0]).await;
    let before = size(&beats[1].beat);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let other_beating = size(&beats[1].beat) > before;

    let terminated = Command::new("kill")
        .args(["-TERM", &second.core.id().to_string()])
        .status()
        .is_ok_and(|status| status.success());
    let Beating {
        mut core,
        _rpc,
        _server,
    } = second;
    let exited = tokio::task::spawn_blocking(move || core.wait())
        .await
        .expect("没 panic");
    let two_stopped = stopped(&beats[1]).await;
    assert!(one_stopped, "硬杀的核心的命令还在跑");
    assert!(other_beating, "另一个核心的命令跟着停了");
    assert!(terminated && exited.is_ok(), "{exited:?}");
    assert!(two_stopped, "有计划地停下以后命令还在跑");
    assert!(gone(group.trim()), "看门的 {group} 留下了");
}
