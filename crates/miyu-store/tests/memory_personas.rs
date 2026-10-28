//! 账号名下有记忆日志、回合库的几个人格（施工 R-25：`miyu check` 照它数补到了哪）。

use miyu_store::memory::MemoryLogs;
use miyu_store::recall::{RecallIndexes, Room};

use crate::support::*;

/// 有记忆日志的几个人格（施工 R-25）：开过的、目录在的照编号排；什么都没有的是空的。回合库同样照文件列。
#[test]
fn the_personas_with_a_log_or_turns_are_listed() {
    let scratch = Scratch::new("memory-personas");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    assert_eq!(logs.personas().expect("列得出"), Vec::<String>::new());
    for persona in ["miyu", "engineer"] {
        logs.open(&Room::persona(&admin(), persona))
            .expect("开得了");
    }
    assert_eq!(logs.personas().expect("列得出"), ["engineer", "miyu"]);
    let turns = RecallIndexes::new(&root);
    assert_eq!(turns.personas().expect("列得出"), Vec::<String>::new());
    turns.turns(&Room::persona(&admin(), "miyu"));
    assert_eq!(turns.personas().expect("列得出"), ["miyu"]);
}
