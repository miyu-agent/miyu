//! 导进来的记忆进检索库（施工 MI-1，`docs/blueprint/memory.md` 第二条）：记忆库那一行的时刻照原来记下的 `noted`，不照写进
//! 日志的那一刻：记忆库已经建了、接着放进去的，和删了重开、照底账补回来的都是。

use std::fs;

use miyu_kernel::origin::By;
use miyu_recall::{MemoryEvent, Origin, Saved};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::Room;

use crate::support::*;

fn person() -> By {
    serde_json::from_str(r#"{"kind":"person"}"#).expect("合写法")
}

fn saved(text: &str, imported: Option<Origin>) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
        imported: imported.map(Box::new),
    })
}

fn imported(text: &str, origin: &str) -> MemoryEvent {
    saved(
        text,
        Some(Origin {
            noted: at(1),
            origin: origin.into(),
        }),
    )
}

fn room() -> Room {
    Room::persona("engineer")
}

#[test]
fn an_imported_memory_is_indexed_at_when_it_was_noted() {
    let scratch = Scratch::new("memory-imported");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room()).expect("开得了").0;
    // 先记一条别的：记忆库建起来，导进来的那一条照追加的那一路放进去，不是建库时照底账整份放。
    let plain = log
        .append(at(40), person(), None, &saved("用户住北京", None))
        .expect("记得下")
        .id;
    let id = log
        .append(at(50), person(), None, &imported("用户养猫", "o:1"))
        .expect("记得下")
        .id;
    let expected = [
        (plain.to_string(), "用户住北京".to_string(), at(40)),
        (id.to_string(), "用户养猫".to_string(), at(1)),
    ];
    let mut rows = log.index().entries().expect("读得了");
    rows.sort();
    assert_eq!(rows, expected, "照原来记下的时刻，别的照事件的时刻");
    drop((log, logs));
    let index = root.index().join("recall/memory-engineer.db");
    for suffix in ["", "-wal", "-shm"] {
        let mut name = index.clone().into_os_string();
        name.push(suffix);
        match fs::remove_file(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("{error}"),
        }
    }
    let log = MemoryLogs::new(&root).open(&room()).expect("开得了").0;
    let mut rows = log.index().entries().expect("读得了");
    rows.sort();
    assert_eq!(rows, expected, "照底账补回来的也照原来的时刻");
}
