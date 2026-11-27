//! 正在造会话时一次停的信号就退（施工 CS-9，`internal/blueprint/core.md`「停下」第 4 条）：真的主程序、真的核心，Unix 上测。
//!
//! 造会话卡住的造法：家目录那一层的人格 `slow` 里，`prompts/reminders.md` 是命名管道。`session.create` 照它找人格时卡在读它
//! 上面，那时拿着会话表的锁（修之前一直拿到 `session.created` 落盘）。这时发一次 `SIGTERM`：停会话不等这把锁，核心照常收尾、
//! 退出码 0；造会话的头收到 `session_stopped`；什么会话都没留下。

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use crate::stopping::exit_within;
use crate::support::{Home, count, within};
use crate::waiting::signal;

/// 家目录那一层的人格 `slow`，找它时读的 `prompts/reminders.md` 是命名管道：交回它。
fn slow_persona(home: &Home) -> std::path::PathBuf {
    let prompts = home.root.personas().join("slow/prompts");
    std::fs::create_dir_all(&prompts).expect("建得了");
    let fifo = prompts.join("reminders.md");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("跑得了");
    assert!(made.success(), "建得了命名管道");
    fifo
}

/// 在连接上发一条请求，不等回应。
async fn send(write: &mut (impl AsyncWriteExt + Unpin), id: &str, method: &str, params: Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    write
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("写得进");
}

/// 读到 `id` 的回应为止（推送跳过）；连接断了的交回空的。
async fn reply(read: &mut (impl AsyncBufReadExt + Unpin), id: &str) -> Option<Value> {
    loop {
        let mut line = String::new();
        if read.read_line(&mut line).await.expect("读得了") == 0 {
            return None;
        }
        let reply: Value = serde_json::from_str(&line).expect("是 JSON");
        if reply["id"] == json!(id) {
            return Some(reply);
        }
    }
}

/// 造会话卡在找人格上（拿着会话表的锁）时收到一次停的信号：几秒内照常收尾，退出码 0，运行日志记一行「停了」，造会话的头
/// 收到 `session_stopped`，没留下会话（修之前停会话一直等这把锁，到了收尾的总时限才强退、退出码 1）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_stop_signal_ends_a_core_creating_a_session() {
    let home = Home::new();
    let fifo = slow_persona(&home);
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let mut core = home
        .core_staying()
        .env("MIYU_TEST_STOP_LIMIT_MS", "20000")
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let (read, mut write) = tokio::io::split(connection);
    let mut read = tokio::io::BufReader::new(read);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    send(&mut write, "h1", "hello", hello).await;
    let said = within("hello 的回应", reply(&mut read, "h1")).await;
    assert!(said.is_some_and(|said| said.get("result").is_some()));
    // 写的一头：核心开始读命名管道时这里才打开得了，打开以后一直拿着，核心读不到头、一直卡着。
    let (opened, reading) = std::sync::mpsc::channel();
    let (done, finish) = std::sync::mpsc::channel::<()>();
    let pipe = fifo.clone();
    let writer = std::thread::spawn(move || {
        let held = std::fs::OpenOptions::new().write(true).open(pipe);
        let _opened = opened.send(());
        let _finish = finish.recv();
        drop(held);
    });
    let params = json!({"cwd": work.to_string_lossy(), "persona": "slow"});
    send(&mut write, "c1", "session.create", params).await;
    reading
        .recv_timeout(Duration::from_secs(60))
        .unwrap_or_else(|_| panic!("造会话没去读人格：{}", home.core_log()));
    std::thread::sleep(Duration::from_millis(200));
    let pid = core.id().to_string();
    let term = Instant::now();
    signal(&pid, "TERM");
    let status = exit_within(&mut core, Duration::from_secs(30));
    let took = term.elapsed();
    let answered = tokio::time::timeout(Duration::from_secs(5), reply(&mut read, "c1")).await;
    let _done = done.send(());
    writer.join().expect("没 panic");
    let log = home.core_log();
    assert_eq!(
        status.and_then(|status| status.code()),
        Some(0),
        "{took:?}：{log}"
    );
    assert!(took < Duration::from_secs(10), "{took:?}：{log}");
    assert_eq!(count(&log, "stopped reason=signal"), 1, "{log}");
    assert_eq!(count(&log, "stopping took too long"), 0, "{log}");
    let answered = answered.ok().flatten();
    let reason = answered
        .as_ref()
        .map(|reply| reply["error"]["data"]["reason"].clone());
    assert_eq!(reason, Some(json!("session_stopped")), "{answered:?}");
    assert!(
        home.root.sessions().expect("读得了").is_empty(),
        "没留下会话"
    );
}
