//! 闹钟上了以后才卸掉人格记忆（施工 R-10，`docs/blueprint/memory.md` 第十一条；施工 BUG-5 改成不赛跑）：响的时候没装就不抽。
//!
//! 先后照运行日志认，不靠睡多久：看到第二轮结束上的闹钟（`memory extraction alarm set`）就卸，等到它响了、记下「没装、跳过」
//! （`memory extraction skipped`）再看假服务器。第二句的回应不等：回应要等这一批后台同步完才交，盘慢时闹钟早就响过了（原来
//! 那样写，CI 上同步一慢就抽了）。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::session::Command;
use miyu_log::{LevelFilter, Memory as Logs};
use miyu_session::testkit::{Play, Script};

use support::extracting::{every_turn, organizer};
use support::meaning::{catalog, chat};
use support::*;

#[tokio::test]
async fn an_alarm_set_before_uninstalling_does_not_extract() {
    let logs = Logs::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        logs.clone(),
        LevelFilter::DEBUG,
        None,
    ));
    let server = Server::start(vec![every_turn("不该有")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "一").await;
    let before = count(&logs, "memory extraction alarm set");
    // 第二句不等回应：它要等这一批同步完。
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "二".to_string(),
        })],
        urgent: false,
        venue: None,
    };
    let second = handle.command(id("cmd-2"), alice(), said);
    let answered = async {
        until(&logs, "memory extraction alarm set", before + 1).await;
        // 第二轮结束、闹钟上了：响之前卸掉。
        home.memory.set_installed(false);
        // 响了：记下「没装、跳过」，或者（抽了的）假服务器收到了请求。
        within("闹钟响", async {
            while count(&logs, "memory extraction skipped why=uninstalled") == 0
                && server.received().is_empty()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
    };
    let (replied, ()) = tokio::join!(within("回应", second), answered);
    replied.expect("会话在跑");
    assert!(server.received().is_empty(), "响的时候没装就不抽");
    stop(&handle).await;
}

/// 运行日志里有几行带着 `what`。
fn count(logs: &Logs, what: &str) -> usize {
    logs.lines()
        .iter()
        .filter(|line| line.contains(what))
        .count()
}

/// 等到运行日志里带着 `what` 的有 `n` 行。
async fn until(logs: &Logs, what: &str, n: usize) {
    within(what, async {
        while count(logs, what) < n {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}
