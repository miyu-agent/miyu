//! 整理顺带重看旧记忆（施工 R-19，`docs/blueprint/memory.md` 第七条第 11 款）：一趟里头一个放得下的批照编号带上轮到的旧记忆，最多 20 条，
//! 合成了在 `merged` 后面记一条 `ext.memory.swept`（轮到哪、几条），下一趟接着，轮完一圈从头来；没合成的不往前走；当相关的交过的
//! 也算看过；人叫的没有新记的照样重看，后台的没有新记的不合。整理记忆的模型是一台假服务器（`org/m`）。

use std::sync::Arc;

use miyu_http::testkit::Server;
use miyu_kernel::id::Seq;
use miyu_kernel::time::UtcOffset;
use miyu_recall::{MemoryEvent, Merged, Summary, Swept, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Dreamed, Keeper, Listener, NotDreamed, Turn};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

/// 顺带重看的那一块的标题（`resources/software/memory/merge/sweep.txt`）。
const SWEEP: &str = "Other older memories to look over again:";

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("从 1 起")
}

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(config, UtcOffset::from_minutes(540).expect("合写法"))
        .await
}

/// 这一间记忆日志里的 `swept`，照先后。
fn swept(home: &Home) -> Vec<Swept> {
    let following = keeper(home).follow(Some(0), 4096).expect("跟得上");
    following
        .filled
        .iter()
        .filter_map(|event| match from_event(event)?.ok()? {
            MemoryEvent::Swept(swept) => Some(swept),
            _ => None,
        })
        .collect()
}

/// 第 `at` 次请求里重看的那一块列了哪几条（编号），没有那一块的是空的。
fn looked_over(server: &Server, at: usize) -> Vec<String> {
    let asked = asked(server, at);
    let Some((_, rest)) = asked.split_once(&format!("{SWEEP}\n")) else {
        return Vec::new();
    };
    rest.lines()
        // 一条一行照 `memory_search/memory.txt`（自带换行），重看的那一块在最后。
        .filter(|line| !line.is_empty())
        .filter_map(|line| line.split(' ').next().map(str::to_string))
        .collect()
}

/// 记 `n` 条互不相干的旧记忆（照关键词搜不到彼此），再照旧的样子记一次合到最后一条、一份摘要：都合过了，往后整理只重看。
fn old_memories(home: &Home, n: usize) {
    for i in 0..n {
        save(home, &format!("old note {i:02}"));
    }
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let upto = seq(u64::try_from(n).expect("放得下"));
    let summary = Summary {
        text: "用户的事。".into(),
        upto,
    };
    // 很久以前合的：后台的合并不卡在间隔上（`memory.merge_every`）。
    let long_ago = miyu_kernel::time::Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合写法");
    log.append(long_ago, alice(), None, &MemoryEvent::Summary(summary))
        .expect("记得下");
    let mark = Merged {
        upto,
        given: 0,
        revised: 0,
        retired: 0,
        failed: false,
    };
    log.append(long_ago, alice(), None, &MemoryEvent::Merged(mark))
        .expect("记得下");
}

fn ids(range: std::ops::RangeInclusive<u64>) -> Vec<String> {
    range.map(|n| format!("m{n}")).collect()
}

#[tokio::test]
async fn each_dream_looks_over_the_next_twenty_and_wraps_around() {
    let server = Server::start(vec![stream("{}"), stream("{}"), stream("{}"), stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    old_memories(&home, 45);
    for _ in 0..3 {
        dream(&home).await.expect("整理了");
    }
    assert_eq!(looked_over(&server, 0), ids(1..=20), "头一趟从第一条起");
    assert_eq!(looked_over(&server, 1), ids(21..=40), "接着上一趟");
    let wrapped: Vec<String> = ids(41..=45).into_iter().chain(ids(1..=15)).collect();
    assert_eq!(looked_over(&server, 2), wrapped, "轮完一圈从头接，不重复");
    assert_eq!(
        swept(&home),
        [
            Swept {
                upto: seq(20),
                count: 20
            },
            Swept {
                upto: seq(40),
                count: 20
            },
            Swept {
                upto: seq(15),
                count: 20
            },
        ]
    );
    assert!(
        !asked(&server, 0).contains("Saved since the summary:"),
        "没有新记的那一块"
    );
}

#[tokio::test]
async fn a_failed_batch_does_not_move_on() {
    let server = Server::start(vec![stream("not json"), stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    old_memories(&home, 3);
    assert!(matches!(dream(&home).await, Err(NotDreamed::Failed(_))));
    assert!(swept(&home).is_empty(), "没合成的不记");
    dream(&home).await.expect("整理了");
    assert_eq!(looked_over(&server, 1), ids(1..=3), "还从头来");
    assert_eq!(swept(&home).len(), 1);
}

#[tokio::test]
async fn new_memories_come_first_and_the_first_batch_with_room_looks_over() {
    let server = Server::start(vec![stream("{}"), stream("{}"), stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    // 旧的也长：头一批让新记的塞满了，剩下的地方放不下一条旧的（英文，和新记的照关键词搜不到彼此）。
    for i in 0..2 {
        save(
            &home,
            &format!("old note {i:02}: {}", "a long sentence ".repeat(30)),
        );
    }
    old_memories_after(&home, 2);
    for n in 0..100 {
        save(
            &home,
            &format!("第 {n} 条新的：{}", "很长的一句话".repeat(19)),
        );
    }
    dream(&home).await.expect("整理了");
    assert_eq!(server.received().len(), 2, "放不下的分两批");
    assert!(
        looked_over(&server, 0).is_empty(),
        "头一批让新记的塞满了，往后挪"
    );
    let second = asked(&server, 1);
    assert!(
        second.find("Saved since the summary:") < second.find(SWEEP),
        "新记的在前：{second}"
    );
    assert_eq!(looked_over(&server, 1), ids(1..=2));
    assert_eq!(
        swept(&home),
        [Swept {
            upto: seq(2),
            count: 2
        }],
        "一趟只重看一次，照开头挑好的那两条：不把这一趟刚合完的新记的当成轮到的"
    );
}

#[tokio::test]
async fn a_related_one_already_given_counts_as_looked_over() {
    let server = Server::start(vec![stream("{}"), stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    old_memories_after(&home, 1);
    save(&home, "用户的猫叫团子");
    dream(&home).await.expect("整理了");
    let first = asked(&server, 0);
    assert!(first.contains("Older related memories:\nm1 "), "{first}");
    assert!(looked_over(&server, 0).is_empty(), "交过了不重复列");
    assert_eq!(
        swept(&home),
        [Swept {
            upto: seq(1),
            count: 0
        }],
        "当相关的看过了，照样往前走"
    );
}

#[tokio::test]
async fn a_background_merge_with_nothing_new_asks_nobody() {
    let server = Server::start(vec![
        stream(&serde_json::json!({"memories": []}).to_string()),
        // 多备一个：要是合了，看得出来。
        stream("{}"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    old_memories(&home, 3);
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "你好").await;
    chat(&home, &handle, 2, "在吗").await;
    requested(&server, 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(
        server.received().len(),
        1,
        "只抽了一次：后台的没有新记的不合"
    );
    assert!(swept(&home).is_empty());
    stop(&handle).await;
}

/// 已经记了的前 `n` 条照旧的样子合过了（同 [`old_memories`]，只是不另记）。
fn old_memories_after(home: &Home, n: u64) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let mark = Merged {
        upto: seq(n),
        given: 0,
        revised: 0,
        retired: 0,
        failed: false,
    };
    log.append(now(), alice(), None, &MemoryEvent::Merged(mark))
        .expect("记得下");
}
