//! 到点交进来的话的标签进快照（施工 LF-2）：人生那一对两格都有才交给组装器；LF-2 以前造的快照没有这两格，读得回、写回去
//! 一个字节不变，字节和加这两格以前的写法一样（指纹不变）。

use super::*;
use crate::snapshot::Snapshot;
use crate::test_support::engineer;

/// 加人生那一对以前的四格。
fn four() -> ScheduleTexts {
    ScheduleTexts {
        schedule_open: "<task {id}>\n".to_string(),
        schedule_close: "</task>\n".to_string(),
        loop_open: "<wakeup {id}>\n".to_string(),
        loop_close: "again?\n</wakeup>\n".to_string(),
        life_open: None,
        life_close: None,
    }
}

#[test]
fn the_life_pair_reaches_the_assembler_only_when_both_are_there() {
    let mut texts = four();
    assert!(texts.rendered().unwrap().life.is_none(), "两格都没有");
    texts.life_open = Some("<life {id}>\n".to_string());
    assert!(texts.rendered().unwrap().life.is_none(), "只有一格");
    texts.life_close = Some("sleep?\n</life>\n".to_string());
    let (open, close) = texts.rendered().unwrap().life.expect("两格都有");
    assert_eq!(
        open.render(&BTreeMap::from([("id", "t3")])).unwrap(),
        "<life t3>\n"
    );
    assert_eq!(close, "sleep?\n</life>\n");
}

#[test]
fn a_broken_life_tag_is_named() {
    let mut texts = four();
    texts.life_open = Some("<life {who}>\n".to_string());
    texts.life_close = Some("</life>\n".to_string());
    assert!(matches!(
        texts.rendered(),
        Err(BuildError::Texts {
            which: "schedule message texts",
            ..
        })
    ));
}

#[test]
fn a_snapshot_made_before_lf2_keeps_its_bytes_and_hash() {
    // 加这两格以前 `core.schedule` 写出来就是这四格，一个字节不多。
    let before = r#"{"schedule_open":"<task {id}>\n","schedule_close":"</task>\n","loop_open":"<wakeup {id}>\n","loop_close":"again?\n</wakeup>\n"}"#;
    assert_eq!(serde_json::to_string(&four()).unwrap(), before);
    let read: ScheduleTexts = serde_json::from_str(before).unwrap();
    assert_eq!(read, four(), "没有这两格的读成没有");

    let mut older = engineer();
    older.core.schedule = Some(four());
    let bytes = older.to_bytes();
    assert!(!String::from_utf8(bytes.clone()).unwrap().contains("life_"));
    let back = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(back, older);
    assert_eq!(back.to_bytes(), bytes, "读进来再写出去一个字节不变");
    assert_eq!(back.hash(), older.hash(), "指纹不变");
    assert!(back.policy().is_ok());
}

#[test]
fn a_new_snapshot_carries_the_life_pair_and_reads_it_back() {
    let mut made = engineer();
    let mut texts = four();
    texts.life_open = Some("<life {id}>\n".to_string());
    texts.life_close = Some("sleep?\n</life>\n".to_string());
    made.core.schedule = Some(texts);
    let bytes = made.to_bytes();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains(r#""life_open":"<life {id}>\n""#), "{text}");
    let back = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(back, made);
    assert!(back.policy().is_ok());
}
