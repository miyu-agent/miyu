//! 扩展驱动交了清单 `hosted` 里没声明的供应商侧工具（设计 71 第三节第 7 条）：核心照收、照落盘，记一行
//! `WARN driver hosted tool undeclared`，带包、驱动、工具名；同一个驱动同一个名字一个进程里只记一次，连说两轮也只一行；
//! 同一条回复里声明了的那一块不记。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use serde_json::json;

use miyu_http::testkit::Server;
use miyu_log::{LevelFilter, Memory};
use support::extensions::*;
use support::hosted::*;
use support::*;

#[tokio::test]
async fn an_undeclared_hosted_tool_is_kept_and_warned_about_once() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::TRACE,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let server = Server::start(Vec::new()).await;
    let home = Home::new();
    let program = Program::new();
    install_hosted(
        &home,
        &program.name(),
        &steps(&["hello", "driver:hosted-undeclared", "serve"]),
    );
    let (mut client, core) = connected(&home, &server).await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "titles": false}))
        .await;
    say(&mut client, &home, &session, "alpha", 1).await;
    say(&mut client, &home, &session, "beta", 2).await;
    for blocks in replies(&home, &session) {
        assert_eq!(
            (&blocks[0]["name"], &blocks[1]["name"]),
            (&json!("search_web"), &json!("browse_page")),
            "照收、照落盘：{blocks}"
        );
    }
    assert_eq!(replies(&home, &session).len(), 2);
    let lines = memory.lines();
    let warned: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains("driver hosted tool undeclared"))
        .collect();
    assert_eq!(
        warned.len(),
        1,
        "一个进程只记一次，声明了的 search_web 不记：{lines:#?}"
    );
    assert!(
        warned[0].contains(" WARN ")
            && warned[0]
                .ends_with("driver hosted tool undeclared package=ag driver=ag tool=browse_page"),
        "{warned:?}"
    );
    core.stop_extensions().await;
}
