//! 有记忆日志、回合库的几个人格（施工 R-25：`miyu check` 照它数补到了哪）。

use miyu_kernel::origin::By;
use miyu_recall::{Card, MemoryEvent};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::{RecallIndexes, Room};

use crate::support::*;

/// 有记忆日志的几个人格（施工 R-25）：目录在的照编号排；什么都没有的是空的。施工 V-28 起只开不写的不建目录，不算；写过一条
/// 的才有。回合库同样照文件列。
#[test]
fn the_personas_with_a_log_or_turns_are_listed() {
    let scratch = Scratch::new("memory-personas");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    assert_eq!(logs.personas().expect("列得出"), Vec::<String>::new());
    let by: By = serde_json::from_str(r#"{"kind":"person"}"#).expect("合写法");
    for persona in ["miyu", "engineer"] {
        logs.open(&Room::persona(persona)).expect("开得了");
    }
    assert_eq!(
        logs.personas().expect("列得出"),
        Vec::<String>::new(),
        "只开不写的不建目录"
    );
    for persona in ["miyu", "engineer"] {
        let (log, _) = logs.open(&Room::persona(persona)).expect("开得了");
        let card = MemoryEvent::Card(Card {
            who: "qq:10086".into(),
            text: "老朋友".into(),
            names: Vec::new(),
        });
        log.append(at(1), by.clone(), None, &card).expect("记得下");
    }
    assert_eq!(logs.personas().expect("列得出"), ["engineer", "miyu"]);
    let turns = RecallIndexes::new(&root);
    assert_eq!(turns.personas().expect("列得出"), Vec::<String>::new());
    turns.turns(&Room::persona("miyu"));
    assert_eq!(turns.personas().expect("列得出"), ["miyu"]);
}
