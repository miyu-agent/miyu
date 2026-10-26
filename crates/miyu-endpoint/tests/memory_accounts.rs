//! 扩展调 `memory.*`、订阅记忆流照替谁干活找那一间（施工 Y-7，设计 06 U21）：平台桥（声明了系统账号）的记忆归机主，没声明的
//! 扩展替机主干活；碰的都是机主那一间，不是包自己的账号那一间。

use std::time::Duration;

use serde_json::Value;

use miyu_kernel::origin::By;
use miyu_session::testkit::Script;

use crate::support::extensions::*;
use crate::support::*;
use crate::system_account::{replies, served_core};

/// 扩展调 `memory.*` 照替谁干活找那一间（施工 Y-7）：没声明系统账号的包替机主干活，平台桥的记忆归机主；都不是包自己的账号那一间。
#[tokio::test]
async fn extensions_reach_the_owner_s_memory_not_their_own_account_s() {
    let home = Home::new();
    // 核心开记忆日志以前，往机主那一间写进一条。
    {
        let (log, _) = miyu_store::memory::MemoryLogs::new(&home.root)
            .open(&miyu_store::recall::Room::persona(&alice(), "engineer"))
            .expect("开得了");
        let saved = miyu_recall::Saved {
            class: "user".into(),
            text: "主人养了一只猫".into(),
            sources: Vec::new(),
            audience: Vec::new(),
            replaces: None,
            about: None,
            venue: None,
            keywords: None,
            who: None,
            people: Vec::new(),
        };
        let at = miyu_kernel::time::Timestamp::parse("2026-10-23T08:00:00.000Z").expect("合写法");
        log.append(
            at,
            By::Kernel,
            None,
            &miyu_recall::MemoryEvent::Saved(saved),
        )
        .expect("记得下");
    }
    let program = Program::new();
    let listed = r#"ask:memory.list:{"persona":"engineer"}"#;
    let follow = r#"ask:subscribe:{"stream":"memory","persona":"engineer"}"#;
    let (bot_path, bot_step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[&bot_step, "hello", listed, follow, "wait"]),
        "always",
        true,
    );
    let (plain_path, plain_step) = record(&home, "plain");
    install_serving(
        &home,
        "plain",
        &program.name(),
        &steps(&[&plain_step, "hello", listed, follow, "wait"]),
        "always",
        false,
    );
    let core = served_core(&home, &Script::new([]));
    for (who, path) in [("bot", &bot_path), ("plain", &plain_path)] {
        let got = replies(path, 2).await;
        assert_eq!(got[0]["result"]["account"], who, "握手：包自己的账号");
        let texts: Vec<&str> = got[1]["result"]["memories"]
            .as_array()
            .unwrap_or_else(|| panic!("{who} 列得出：{got:?}"))
            .iter()
            .filter_map(|memory| memory["text"].as_str())
            .collect();
        assert_eq!(texts, ["主人养了一只猫"], "{who} 碰的是机主那一间");
        // 订阅记忆日志也是机主那一间：补到它记下的那一条。
        // 回应排在补的后面：等到有 `upto` 的那一行。
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let upto = loop {
            let lines: Vec<Value> = read(path)
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect();
            if let Some(upto) = lines
                .iter()
                .find_map(|line| line["result"]["upto"].as_u64())
            {
                break upto;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "{who} 订阅不上：{lines:?}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        assert!(upto > 0, "{who} 订阅的是机主那一间，补到了那一条");
    }
    core.stop_extensions().await;
}
