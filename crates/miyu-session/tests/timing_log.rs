//! 一次模型请求每一段用了多久的那一行（施工 YC-14，`docs/blueprint/session/actor.md`「运行日志」的 `request timed`）：`DEBUG`
//! 开着，每发一次主请求记一行，格都在；`INFO` 时不记。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

mod support;

use miyu_http::testkit::Server;
use miyu_log::{LevelFilter, Memory};
use support::routing::{hellos, served};
use support::{Home, ask, say, until_turn_ends, watch};

/// 一行里 `key=` 后面的那个数。
fn number(line: &str, key: &str) -> Option<u64> {
    let rest = line.split_once(&format!(" {key}="))?.1;
    rest.split(' ').next()?.parse().ok()
}

#[tokio::test]
async fn each_request_is_timed_once_at_debug_and_not_at_info() {
    let memory = Memory::new();
    let (subscriber, levels) =
        miyu_log::reloadable(memory.clone(), LevelFilter::INFO, String::new, None);
    let _listening = tracing::subscriber::set_default(subscriber);
    // 主请求三次，答完了起标题的也来要：多备几份。
    let server = Server::start(hellos(8)).await;
    let (models, configs) = served(&server.base_url, serde_json::json!({}), "");
    let mut home = Home::new();
    home.configs = configs;
    let handle = home.create(&models).await;
    let mut pushes = watch(&handle).await;

    // `INFO`：一行都不记。
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let quiet = memory.lines();
    assert!(
        quiet.iter().any(|line| line.contains(" request seen=")),
        "请求发过了：{quiet:#?}"
    );
    assert!(
        !quiet.iter().any(|line| line.contains("request timed")),
        "`INFO` 时不记：{quiet:#?}"
    );

    // `DEBUG`：两次主请求，各一行。
    levels.set(LevelFilter::DEBUG);
    for id in ["cmd-2", "cmd-3"] {
        ask(&handle, id, say("hi")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    let lines = memory.lines();
    let session = handle.id().as_str();
    let head = format!(" DEBUG session  {session} request timed seen=");
    // 带 `kernel_us` 的是主请求的；起标题那样的辅助请求也记一行，没有内核那几格。
    let timed: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains(&head) && line.contains(" kernel_us="))
        .collect();
    let asked: Vec<u64> = lines[quiet.len()..]
        .iter()
        .filter(|line| line.contains(&format!(" INFO  session  {session} request seen=")))
        .filter_map(|line| number(line, "seen"))
        .collect();
    assert_eq!(asked.len(), 2, "`DEBUG` 以后发了两次主请求：{lines:#?}");
    let seen: Vec<u64> = timed
        .iter()
        .filter_map(|line| number(line, "seen"))
        .collect();
    assert_eq!(seen, asked, "每次主请求一行，照先后：{lines:#?}");
    for line in timed {
        for key in [
            "kernel_us",
            "assemble_us",
            "estimate_us",
            "fingerprint_us",
            "messages",
            "wait_us",
            "blobs_us",
            "encode_us",
            "drop_us",
            "bytes",
        ] {
            assert!(number(line, key).is_some(), "`{key}` 应该在「{line}」里");
        }
        assert!(number(line, "bytes") > Some(0), "请求体有字节：{line}");
        assert!(number(line, "messages") > Some(0), "请求里有消息：{line}");
        // 那一行紧挨着 HTTP 的 `sent`，请求体一样大。
        let bytes = number(line, "bytes").unwrap_or_default();
        let sent = format!(" DEBUG http     {session} sent host=127.0.0.1 bytes={bytes}");
        assert!(
            lines.iter().any(|line| line.ends_with(&sent)),
            "「{sent}」应该在 {lines:#?} 里"
        );
        println!("{line}");
    }
}
