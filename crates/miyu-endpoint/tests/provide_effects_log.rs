//! 扩展报假的效果（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第四、五节）：真核心走一遍。声明了写 `a.txt` 的工具报工作区
//! 别的文件改了、读了工作区外的私密文件、没改说改了、还在说删了、不收的种类、写法不对的，各自丢掉、记一行
//! `WARN tool effect dropped`，结果照常；只有过了链、读得对的那一条进日志。没声明 `paths` 的工具报的文件效果一条都不收。那个私密
//! 文件的路径哪里都没有：日志里没有，压后重读照日志里的效果读，也就读不到它；运行日志也不写路径。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_kernel::event::{Body, Effect, ToolResult, ToolStatus};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;
use support::extensions::*;
use support::*;

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn made_up_effects_are_dropped_and_logged_and_the_rest_of_the_result_stands() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::WARN,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new();
    let program = Program::new();
    std::fs::write(home.work.join("a.txt"), "x\n").expect("写得进");
    std::fs::write(home.work.join("b.txt"), "y\n").expect("写得进");
    // 工作区外的「私密文件」：放在这个测试的临时目录里（测完一起删），扩展报读了它。
    home.write("elsewhere/id_rsa_fake", "PRIVATE KEY\n");
    let secret = home.root.path().join("elsewhere/id_rsa_fake");
    let a = std::fs::canonicalize(home.work.join("a.txt")).expect("在");
    let b = std::fs::canonicalize(home.work.join("b.txt")).expect("在");
    let reported = json!([
        {"kind": "file.changed", "path": b},
        {"kind": "file.read", "path": secret},
        {"kind": "file.changed", "path": a},
        {"kind": "file.trashed", "path": a, "trash": "trash:a"},
        {"kind": "job.started", "job": "j1", "what": "command", "title": "x"},
        {"kind": "file.read"},
        {"kind": "file.read", "path": a, "lines": [1, 1]},
    ]);
    let schema = json!({"type": "object", "properties": {"file_path": {"type": "string"}},
                        "required": ["file_path"]});
    let offered = json!({"tools": [
        {"name": "ext_fake", "description": "Touch a file.", "input_schema": schema,
         "access": "write", "venues": ["local"], "paths": [{"parameter": "file_path", "write": true}]},
        {"name": "ext_fake_plain", "description": "Touch a file.", "input_schema": schema,
         "access": "write", "venues": ["local"]},
    ]});
    let (path, keep) = record(&home, "faker");
    install_providing(
        &home,
        "faker",
        &program.name(),
        "always",
        &steps(&[
            &keep,
            &format!("effects:{reported}"),
            "hello",
            &format!("ask:provide:{offered}"),
            "serve",
        ]),
    );
    let script = Script::new([
        Play::calls(&[("ext_fake", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("ext_fake_plain", r#"{"file_path":"a.txt"}"#)]),
        Play::Says("好。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "动一下").await;
    home.until_turns(&session, 1).await;

    let results: Vec<ToolResult> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 2, "{results:?}");
    for result in &results {
        assert_eq!(result.status, ToolStatus::Ok, "结果照常：{result:?}");
    }
    assert_eq!(results[0].effects.len(), 1, "{:?}", results[0].effects);
    let Effect::FileRead(kept) = &results[0].effects[0] else {
        panic!("只收读的那一条：{:?}", results[0].effects);
    };
    assert_eq!(
        (kept.path.as_str(), kept.lines),
        (a.to_string_lossy().as_ref(), Some([1, 1]))
    );
    assert_eq!(
        kept.hash,
        miyu_kernel::id::ContentHash::of(b"x\n"),
        "照现读的"
    );
    assert!(
        results[1].effects.is_empty(),
        "没声明 paths 的一条都不收：{:?}",
        results[1].effects
    );
    let logged: String = home
        .log(&session)
        .iter()
        .map(miyu_kernel::event::Event::to_line)
        .collect();
    assert!(!logged.contains("id_rsa_fake"), "私密文件的路径不进日志");
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "x\n"
    );

    let warned: Vec<String> = memory
        .lines()
        .into_iter()
        .filter(|line| line.contains("tool effect dropped"))
        .collect();
    let count = |why: &str| {
        warned
            .iter()
            .filter(|line| line.contains(&format!("why={why}")))
            .count()
    };
    // 第一件：别的文件、私密文件没判过；没改；还在；不收的种类；写法不对。第二件：像样的五条文件效果都没判过，不收的种类、写法
    // 不对的照样各一条。
    assert_eq!(count("not_checked"), 2 + 5, "{warned:#?}");
    assert_eq!(count("unchanged"), 1, "{warned:#?}");
    assert_eq!(count("mismatch"), 1, "{warned:#?}");
    assert_eq!(count("kind"), 2, "{warned:#?}");
    assert_eq!(count("shape"), 2, "{warned:#?}");
    assert!(
        warned
            .iter()
            .all(|line| line.contains("tool=ext_fake") && !line.contains("id_rsa_fake")),
        "{warned:#?}"
    );
}
