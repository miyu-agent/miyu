//! 抽取读完以后、记下以前撤销了那一轮（施工 E-5 修）：抽取照读的那一刻的快照算，撤销的那一轮照样记下；出处死了，列不出、
//! 搜不到，恢复了又看得到（`docs/blueprint/memory.md` 第三条第 4 款：出处活不活照回合库里的墓碑判）。不另在落盘前核对。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::id::TurnId;
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;
use miyu_session::Filter;
use miyu_session::testkit::{Play, Script};

use crate::support::dreaming::keeper;
use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

/// 列得出的、抽取记下的那几条出自哪几轮，照先后。
fn shown_turns(home: &Home) -> Vec<TurnId> {
    home.memory.settle();
    let mut turns: Vec<TurnId> = keeper(home)
        .list(&Filter {
            limit: 50,
            ..Filter::default()
        })
        .expect("列得出")
        .into_iter()
        .filter(|entry| matches!(entry.by, By::Module(_)))
        .flat_map(|entry| entry.sources.into_iter().map(|source| source.turn))
        .collect();
    turns.sort();
    turns.dedup();
    turns
}

#[tokio::test]
async fn a_turn_undone_after_the_read_is_saved_but_not_shown() {
    // 抽取的回答停在闸前：读完、拼好、发出去了才撤销第二轮，再放回答过去。
    let gate = Arc::new(tokio::sync::Barrier::new(2));
    let mut body = vec![Piece::Gate(Arc::clone(&gate))];
    body.extend(every_turn("用户在聊天").body);
    let server = Server::start(vec![Reply::stream(body)]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "第一句").await;
    chat(&home, &handle, 2, "第二句").await;
    requested(&server, 1).await;
    let turns = ended_turns(&home, handle.id());
    assert_eq!(turns.len(), 2);
    assert!(
        asked(&server, 0).contains("第二句"),
        "读的那一刻第二轮还在：{}",
        asked(&server, 0)
    );
    let undone = ask(
        &handle,
        "cmd-3",
        Command::Revert {
            turn: Some(turns[1]),
        },
    )
    .await;
    assert!(undone.is_ok(), "{undone:?}");
    gate.wait().await;
    for _ in 0..200 {
        if extracted(&home).len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let saved: Vec<TurnId> = extracted(&home)
        .into_iter()
        .flat_map(|entry| entry.sources.into_iter().map(|source| source.turn))
        .collect();
    assert!(
        saved.contains(&turns[0]) && saved.contains(&turns[1]),
        "照读的那一刻的快照，两轮都记下了：{saved:?}"
    );
    assert_eq!(shown_turns(&home), [turns[0]], "撤销的那一轮记下了也列不出");

    let restored = ask(&handle, "cmd-4", Command::Unrevert).await;
    assert!(restored.is_ok(), "{restored:?}");
    assert_eq!(shown_turns(&home), [turns[0], turns[1]], "恢复了又看得到");
    stop(&handle).await;
}
