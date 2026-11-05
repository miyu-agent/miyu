//! 扩展发了超长的一行（施工 FX-1，接入通讯平台报的：表情包把整张图塞进一条 `blob.put`）：核心只拒这一条，连接照旧，下一条
//! 照常回；不断开，扩展也就不会读到头退出、被当成失败一次次重启，最后停成 `failed_repeatedly`。连接那一层的测试在 FX-1 自己的
//! 步子里；这里是真核心拉起真进程的那一层。夹具同 `tests/extensions.rs`（`support/extensions.rs`）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::support::extensions::*;
use crate::support::*;

/// 超过一行的上限（`wire::LINE_LIMIT`，1 MiB）一截。
const TOO_LONG: usize = 1024 * 1024 + 4096;

/// 等到 `path` 里记下的有 `n` 行，交回它们。
async fn lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<String> = read(path).lines().map(str::to_string).collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_line_too_long_is_refused_alone_and_the_connection_goes_on() {
    let home = Home::new();
    let program = Program::new();
    let (path, keep) = record(&home, "echo");
    let long = format!("long:{TOO_LONG}");
    install(
        &home,
        "echo",
        &program.name(),
        "manual",
        &steps(&[keep.as_str(), "hello", &long, "call:model.list", "wait"]),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    call(&mut client, "extension.enable", "echo").await;

    // 记下的：工作目录、握手的回应、超长那一条的回应、下一条的回应。
    let got = lines(&path, 4).await;
    assert!(got[0].starts_with("cwd:"), "{got:?}");
    let refused: Value =
        serde_json::from_str(&got[2]).unwrap_or_else(|_| panic!("超长那一条有回应：{got:?}"));
    assert!(refused.get("error").is_some(), "超长那一条被拒：{refused}");
    let next: Value =
        serde_json::from_str(&got[3]).unwrap_or_else(|_| panic!("下一条有回应：{got:?}"));
    assert!(
        next.get("result").is_some(),
        "超长那一条以后连接照旧，下一条照常回：{next}"
    );
    assert!(
        got[4..].iter().all(|line| !line.starts_with("cwd:")),
        "没有被重新拉起：{got:?}"
    );

    let running = until_state(&mut client, "echo", |one| one["state"] == "running").await;
    assert_eq!(running["failures"], 0, "不算失败：{running}");
    // 过一会儿再看一次：没有断开、读到头退出、退避重启。
    tokio::time::sleep(Duration::from_millis(500)).await;
    let still = status(&mut client, "echo").await;
    assert_eq!(still["state"], "running", "{still}");
    assert_eq!(still["failures"], 0, "{still}");
    assert_eq!(
        read(&path)
            .lines()
            .filter(|line| line.starts_with("cwd:"))
            .count(),
        1,
        "只拉起了一次"
    );
}
