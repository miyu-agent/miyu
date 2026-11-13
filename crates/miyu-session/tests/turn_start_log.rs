//! 回合开始的挂接点每一段等了多久的那一行（施工 PF-3，`docs/blueprint/session/actor.md`「运行日志」的 `turn start timed`）：
//! `DEBUG` 开着，每个回合一行，几段都是数；`INFO` 时不记。
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

/// 每个回合都有的几格：回合编号、actor 自己等的几段、一共多久。挂接点里的几件问了才有。
const ALWAYS: &[&str] = &[
    "turn",
    "config_us",
    "config_wait_us",
    "persona_us",
    "persona_wait_us",
    "skills_us",
    "skills_wait_us",
    "line_us",
    "line_wait_us",
    "extensions_us",
    "project_us",
    "project_wait_us",
    "total_us",
];

/// 这里的会话开着记忆、人说了话：摘要、卡、联想都问了。知识库没装，没有那两格。
const MEMORY: &[&str] = &[
    "memory_us",
    "summary_us",
    "summary_wait_us",
    "card_us",
    "card_wait_us",
    "recall_us",
    "recall_wait_us",
];

#[tokio::test]
async fn each_turn_start_is_timed_once_at_debug_and_not_at_info() {
    let memory = Memory::new();
    let (subscriber, levels) =
        miyu_log::reloadable(memory.clone(), LevelFilter::INFO, String::new, None);
    let _listening = tracing::subscriber::set_default(subscriber);
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
        "回合跑过了：{quiet:#?}"
    );
    assert!(
        !quiet.iter().any(|line| line.contains("turn start timed")),
        "`INFO` 时不记：{quiet:#?}"
    );

    // `DEBUG`：两个回合，各一行。
    levels.set(LevelFilter::DEBUG);
    for id in ["cmd-2", "cmd-3"] {
        ask(&handle, id, say("hi")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    let lines = memory.lines();
    let session = handle.id().as_str();
    let head = format!(" DEBUG session  {session} turn start timed turn=");
    let timed: Vec<&String> = lines.iter().filter(|line| line.contains(&head)).collect();
    assert_eq!(timed.len(), 2, "两个回合各一行：{lines:#?}");
    let turns: Vec<u64> = timed
        .iter()
        .filter_map(|line| number(line, "turn"))
        .collect();
    assert!(
        turns.len() == 2 && turns[0] < turns[1],
        "照先后，回合编号往前走：{turns:?}"
    );
    for line in timed {
        for key in ALWAYS.iter().chain(MEMORY) {
            assert!(number(line, key).is_some(), "`{key}` 应该在「{line}」里");
        }
        assert!(!line.contains(" knowledge_us="), "没问知识库：{line}");
        // 挂接点里问了的那几件也是数；一格都不许是别的写法。
        let rest = line
            .split_once(" turn start timed")
            .map_or("", |(_, rest)| rest);
        for pair in rest.split_whitespace() {
            let (key, value) = pair.split_once('=').expect("键=值");
            assert!(value.parse::<u64>().is_ok(), "`{key}` 是数：{line}");
        }
        // 冻结配置那一段只有一趟阻塞线程：来回等的不会比整段长。
        assert!(
            number(line, "config_wait_us") <= number(line, "config_us"),
            "等的在整段里头：{line}"
        );
        assert!(
            number(line, "config_us") <= number(line, "total_us"),
            "一段在一共里头：{line}"
        );
        println!("{line}");
    }
}
