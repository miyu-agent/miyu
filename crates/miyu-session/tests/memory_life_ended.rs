//! 结束了的人生会话不进回合索引（施工 LF-4 上，`docs/blueprint/memory.md` 第一条第 7、9 款）：回合库删了重建、后台补齐旧会话时
//! 照日志判，日志里有 `life.ended` 的不补；别的会话照补。

use std::time::{Duration, Instant};

use miyu_kernel::event::LifeEnded;
use miyu_kernel::session::{Appended, Command};
use miyu_session::Memory;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::*;

/// 说一句，等这一轮结束；`ended` 的接着记下 `life.ended`，停下。交回会话编号。
async fn said(home: &Home, words: &str, ended: bool) -> miyu_kernel::id::SessionId {
    let lines = Lines {
        indexed: false,
        ..Lines::default()
    };
    let script = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &miyu_tool::Catalog::default(),
            Opening::default(),
            lines,
        )
        .await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    if ended {
        let end = Command::Append {
            event: Appended::LifeEnded(LifeEnded { last_words: None }),
        };
        ask(&handle, "cmd-2", end).await.expect("会话在跑");
    }
    stop(&handle).await;
    handle.id().clone()
}

/// 那一间的回合库里搜 `words`，交回键。
fn found(memory: &Memory, words: &str) -> Vec<String> {
    memory.settle();
    let (index, _) = memory.turns.turns(&Room::persona("engineer"));
    index
        .search(words, 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| hit.key)
        .collect()
}

#[tokio::test]
async fn a_rebuilt_index_leaves_out_an_ended_life() {
    let home = Home::new();
    let cat = said(&home, "我养了一只猫", false).await;
    said(&home, "今天去了海边", true).await;
    let memory = Memory::new(&home.root, None);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let cats = found(&memory, "养了");
        if cats.len() == 1 {
            assert!(cats[0].starts_with(&format!("{cat}/")), "{cats:?}");
            break;
        }
        assert!(Instant::now() < deadline, "等不到补齐：{cats:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(found(&memory, "海边").is_empty(), "结束了的人生不补回来");
}
