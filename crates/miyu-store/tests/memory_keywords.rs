//! 记忆的关键词进检索库（施工 R-15 上，`docs/blueprint/memory.md` 第二条）：带关键词记的照关键词搜得到；换了关键词的照新的搜得到、
//! 旧的搜不到、编号不变，旧的向量拿掉、补的时候照新的字补；记忆库删了重开，带着的、换过的关键词照样补回来；记忆库落在换关键词
//! 那一行前面的（日志里多了它、记忆库还没照到），重开时补上。

use std::fs;

use miyu_kernel::origin::By;
use miyu_recall::{MemoryEvent, MemoryId, Saved, Tagged};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::Room;

use crate::support::*;

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

fn saved(text: &str, keywords: Option<&str>) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: Vec::new(),
        replaces: None,
        about: None,
        venue: None,
        keywords: keywords.map(str::to_string),
        who: None,
        people: Vec::new(),
        place: None,
    })
}

fn tagged(id: MemoryId, keywords: &str) -> MemoryEvent {
    MemoryEvent::Tagged(Tagged {
        id,
        keywords: keywords.into(),
    })
}

fn room() -> Room {
    Room::persona(&admin(), "engineer")
}

#[test]
fn keywords_are_searched_and_a_new_set_replaces_the_old() {
    let scratch = Scratch::new("memory-keywords");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room()).expect("开得了").0;
    let first = log
        .append(
            at(1),
            tool(),
            None,
            &saved("用户用 N 卡", Some("显卡,英伟达")),
        )
        .expect("记得下")
        .id;
    let plain = log
        .append(at(2), tool(), None, &saved("喜欢吃火锅", None))
        .expect("记得下")
        .id;
    assert_eq!(
        log.search("英伟达", 10).expect("搜得了"),
        [first],
        "照关键词搜得到"
    );
    assert_eq!(log.search("火锅", 10).expect("搜得了"), [plain]);

    log.index()
        .put_vector(&first.to_string(), "local:m", &[1.0, 0.0])
        .expect("放得进");
    assert!(
        log.index()
            .missing("local:m", 0, 10)
            .expect("读得了")
            .iter()
            .all(|(_, key, _)| key != &first.to_string())
    );
    let appended = log
        .append(at(3), tool(), None, &tagged(first, "GPU,4090"))
        .expect("记得下");
    assert!(appended.indexed.is_ok());
    assert_eq!(
        log.search("4090", 10).expect("搜得了"),
        [first],
        "换了的照新的搜得到、编号不变"
    );
    assert!(
        log.search("英伟达", 10).expect("搜得了").is_empty(),
        "旧的搜不到"
    );
    let missing = log.index().missing("local:m", 0, 10).expect("读得了");
    let again = missing
        .iter()
        .find(|(_, key, _)| key == &first.to_string())
        .expect("换了字，旧的向量拿掉、要重补");
    assert_eq!(again.2, "用户用 N 卡\nGPU,4090", "照正文接关键词补");
}

#[test]
fn a_rebuilt_memory_index_has_the_latest_keywords() {
    let scratch = Scratch::new("memory-keywords-rebuild");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room()).expect("开得了").0;
    let first = log
        .append(at(1), tool(), None, &saved("用户用 N 卡", Some("显卡")))
        .expect("记得下")
        .id;
    log.append(at(2), tool(), None, &tagged(first, "GPU,4090"))
        .expect("记得下");
    let kept = log
        .append(
            at(3),
            tool(),
            None,
            &saved("喜欢吃东西", Some("火锅,麻辣烫")),
        )
        .expect("记得下")
        .id;
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
    assert_eq!(
        log.search("4090", 10).expect("搜得了"),
        [first],
        "照整份底账补：换过的关键词也在"
    );
    assert!(
        log.search("显卡", 10).expect("搜得了").is_empty(),
        "换掉的不在"
    );
    assert_eq!(
        log.search("火锅", 10).expect("搜得了"),
        [kept],
        "带着的、没换过的关键词也在"
    );
}

#[test]
fn a_tagging_the_memory_index_has_not_seen_is_filled_in() {
    let scratch = Scratch::new("memory-keywords-lagging");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room()).expect("开得了").0;
    let first = log
        .append(at(1), tool(), None, &saved("用户用 N 卡", Some("显卡")))
        .expect("记得下")
        .id;
    drop((log, logs));
    // 日志里多一行换关键词的、记忆库还没照到它（像写完日志、没来得及写记忆库就停了）。
    let segment = root
        .path()
        .join("modules/memory/engineer/000000000001.jsonl");
    let line = miyu_recall::to_event(
        miyu_kernel::id::Seq::new(2).expect("从 1 起"),
        at(2),
        tool(),
        &tagged(first, "GPU,4090"),
    )
    .expect("写得出")
    .to_line();
    let mut bytes = fs::read(&segment).expect("在");
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    fs::write(&segment, bytes).expect("写得进");
    let log = MemoryLogs::new(&root).open(&room()).expect("开得了").0;
    assert_eq!(
        log.search("4090", 10).expect("搜得了"),
        [first],
        "重开时补上换的关键词"
    );
    assert!(
        log.search("显卡", 10).expect("搜得了").is_empty(),
        "换掉的不在"
    );
}
