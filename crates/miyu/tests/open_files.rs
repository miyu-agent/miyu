//! 打开文件数的软上限（施工 YC-1，压测报告 F1）：系统给的软上限低，核心起来时自己调高。软上限压到 64 拉起真的核心，开 50 个
//! 会话、每个跑一条命令，全成，运行日志里没有文件数用光的痕迹。只在 Unix 上：Windows 没有这个上限。

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use serde_json::json;

use crate::crash::Rpc;
use crate::support::{Home, within};

/// 软上限压到 64 再照原样起后面的命令（`$0` 是程序，`$@` 是参数）。硬上限不动，只压软的。
const LOW: &str = "ulimit -S -n 64 && exec \"$0\" \"$@\"";

#[tokio::test]
async fn a_core_started_under_a_low_limit_still_runs_fifty_sessions() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作目录");
    let inner = home.core_staying();
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", LOW])
        .arg(inner.get_program())
        .args(inner.get_args());
    for (key, value) in inner.get_envs() {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }
    let mut core = command.stdout(Stdio::piped()).spawn().expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let mut rpc = Rpc::over(connection);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call("hello-1", "hello", hello).await;
    for n in 0..50 {
        let created = rpc
            .call(
                &format!("create-{n}"),
                "session.create",
                json!({"cwd": work.to_string_lossy()}),
            )
            .await;
        let session = created["session"].as_str().expect("有编号").to_string();
        rpc.call(
            &format!("send-{n}"),
            "session.send",
            json!({"session": session, "text": "true", "shell": true}),
        )
        .await;
    }
    let listed = rpc.call("list-1", "session.list", json!({})).await;
    let log = home.core_log();
    home.kill_core().await;
    #[expect(
        clippy::let_underscore_must_use,
        reason = "核心已经停了，收掉进程，不管它的退出码"
    )]
    let _ = core.wait();
    assert_eq!(
        listed["sessions"].as_array().map(Vec::len),
        Some(50),
        "{listed}"
    );
    for gone in [
        "Too many open files",
        "write failed",
        "unreadable",
        "not looked at",
    ] {
        assert!(!log.contains(gone), "运行日志里有「{gone}」：{log}");
    }
}
