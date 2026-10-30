//! 闲时合回库的写前日志（施工 V-28，`docs/blueprint/store.md`「怎么走」的「库的写前日志」）：真的 `miyu core`，模型是本机
//! 回环上的假服务器。说过两轮、库安静下来以后，数据根里每个库的 `-wal` 都是 0 字节；这时硬杀核心（Unix 上 `SIGKILL`，
//! Windows 上结束进程），库照样开得了、没坏、行都在。不等安静就杀的也一样：崩溃安全和以前一样。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::{Home, MIYU, within};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_store::index::{FILE, SessionIndex};
use miyu_store::recall::{Opened, RecallIndex};

/// 一句回答：照 DeepSeek 的写法，一块 `data:` 一行，最后 `[DONE]`。
fn said(text: &str) -> Reply {
    let chunk: Value = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": text},
        "finish_reason": "stop"}]});
    Reply::stream(vec![Piece::Bytes(
        format!("data: {chunk}\n\ndata: [DONE]\n\n").into_bytes(),
    )])
}

/// 拉起核心（不空闲退出）：模型是 `server`，等它写来 `ready`。
fn start(home: &Home, server: &Server) -> Child {
    let config = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\ncatalog = \"deepseek\"\nkey = {{ env = \"MIYU_TEST_KEY\" }}\n\n[models]\nchat = \"dev/deepseek-chat\"\n",
        server.base_url
    );
    std::fs::write(home.root.path().join("config.toml"), config).expect("写得进");
    let mut core = home
        .core_staying()
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

/// 在开着的核心上跑 `miyu <args>`：在阻塞线程里跑，假服务器在测试的运行时里照常回。
async fn miyu(home: &Home, args: &'static [&'static str]) {
    let root = home.root.path().to_path_buf();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(MIYU)
            .args(args)
            .env("MIYU_HOME", &root)
            .envs(crate::support::offline(&root))
            .env("MIYU_RESOURCES", crate::support::resources())
            .env("MIYU_TEST_KEY", "test")
            .env("NO_PROXY", "127.0.0.1")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .stdin(Stdio::null())
            .output()
            .expect("跑得起来")
    })
    .await
    .expect("没 panic");
    assert!(output.status.success(), "{args:?}：{output:?}");
}

/// 数据根里每个 `-wal` 和它多大，照路径排。
fn wals(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.to_string_lossy().ends_with(".db-wal") {
                let size = entry.metadata().map_or(0, |meta| meta.len());
                found.push((path, size));
            }
        }
    }
    found.sort();
    found
}

/// 硬杀核心、收掉，再看库：会话列表的索引、回合库开得了、没坏（不是删掉重建的），会话那一行在，`said` 里的几句在回合库里
/// （回合库是后台一批批写的：刚说完就杀的，最后一轮可能还没写，下次载入照日志补，不算丢）。
fn kill_and_check(home: &Home, core: &mut Child, said: &[&str]) {
    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
    let (index, opened) = SessionIndex::open(&home.root.index().join(FILE));
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    assert_eq!(index.rows().expect("读得了").len(), 1, "会话那一行在");
    let turns = home.root.index().join("recall/turns-_none.db");
    let (turns, opened) = RecallIndex::open(&turns);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    for words in said {
        assert!(
            !turns.search(words, 10).expect("搜得了").is_empty(),
            "说过的「{words}」在回合库里"
        );
    }
}

#[tokio::test]
async fn quiet_databases_have_empty_wals_and_a_killed_core_loses_nothing() {
    let home = Home::new();
    let server = Server::start((0..8).map(|_| said("好。")).collect()).await;
    let mut core = start(&home, &server);
    miyu(&home, &["ask", "你好"]).await;
    miyu(&home, &["ask", "-c", "再说一句"]).await;
    let merged = within("安静下来以后合回", async {
        loop {
            let found = wals(home.root.path());
            if found.len() >= 3 && found.iter().all(|(_, size)| *size == 0) {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let names: Vec<String> = merged
        .iter()
        .filter_map(|(path, _)| Some(path.file_name()?.to_string_lossy().into_owned()))
        .collect();
    for db in ["sessions.db-wal", "usage.db-wal", "turns-_none.db-wal"] {
        assert!(names.iter().any(|name| name == db), "{db} 在：{names:?}");
    }
    kill_and_check(&home, &mut core, &["你好", "再说一句"]);
}

#[tokio::test]
async fn a_core_killed_before_it_is_quiet_loses_nothing_either() {
    let home = Home::new();
    let server = Server::start((0..8).map(|_| said("好。")).collect()).await;
    let mut core = start(&home, &server);
    miyu(&home, &["ask", "你好"]).await;
    miyu(&home, &["ask", "-c", "再说一句"]).await;
    kill_and_check(&home, &mut core, &["你好"]);
}
