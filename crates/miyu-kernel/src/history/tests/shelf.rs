//! 闲下来收成槽（施工 LW-10 下，`history/shelf.rs`）：放回的和收起来以前一模一样；读回的少一条、换了一条的不放、照旧收着，之后
//! 读回的齐了照常放回；收着时追加的放回以后和一直没收一样；留着一切的不收；收着时什么都不剩；从日志重建的放回也一样。

use super::*;

/// 一段日志读回来：每一条照日志的一行过一遍，同执行器从磁盘读回的。
fn read_back(log: &[Event], from: Seq) -> Vec<Event> {
    log.iter()
        .filter(|event| event.seq >= from)
        .map(|event| Event::from_line(&event.to_line()).unwrap())
        .collect()
}

/// 撤了第二轮、压过一次的日志：检查点、尾巴、放在一边的撤销都有。
fn busy_log() -> Vec<Event> {
    let mut log = two_turns();
    log.push(message(10, ALICE));
    log.extend(turn(11, 10));
    log.extend(compaction_turn(14, 13, 5));
    log.push(message(17, ALICE));
    log.extend(turn(18, 17));
    log.push(reverted(21, &[18]));
    log
}

/// 放回的和收起来以前一样：检查点、之后的、放在一边的撤销一条不差，从哪条读起是记着的最小的序号。
#[test]
fn put_back_is_what_was_shelved() {
    let log = busy_log();
    let mut history = feed(log.clone());
    let before = history.clone();
    assert!(history.checkpoint().is_some() && !history.last_undone().is_empty());
    let taken = history.shelve().expect("平时那一份收得了");
    assert_eq!(
        taken.len(),
        1 + before.events().len() + before.last_undone().len()
    );
    assert!(history.events().is_empty() && history.checkpoint().is_none());
    assert!(history.last_undone().is_empty());
    let from = history.shelved(Seq::new(22).unwrap()).unwrap();
    assert_eq!(from.get(), before.events()[0].seq.get());
    assert!(history.shelve().is_none(), "收着的不再收");
    history.unshelve(&read_back(&log, from)).unwrap();
    assert_eq!(history, before);
    assert_eq!(history.shelved(Seq::new(22).unwrap()), None);
}

/// 读回的少了一条（日志截短了）、有一条换了序号（那一行被换掉了）：不放，照旧收着；之后读回的齐了照常放回。
#[test]
fn a_short_read_leaves_it_shelved() {
    let log = busy_log();
    let mut history = feed(log.clone());
    let before = history.clone();
    history.shelve().unwrap();
    let from = history.shelved(Seq::new(22).unwrap()).unwrap();
    let mut short = read_back(&log, from);
    short.truncate(short.len() - 3);
    let missing = history.unshelve(&short).unwrap_err();
    assert_eq!(
        missing.to_string(),
        format!("the log has no event {} to put back", missing.seq)
    );
    assert_eq!(history.shelved(Seq::new(22).unwrap()), Some(from));
    let mut gap = read_back(&log, from);
    gap.remove(1);
    assert!(history.unshelve(&gap).is_err());
    assert_eq!(history.shelved(Seq::new(22).unwrap()), Some(from));
    history.unshelve(&read_back(&log, from)).unwrap();
    assert_eq!(history, before);
}

/// 收着的时候追加的（读不回来、执行器照旧送了输入）：放回以后照先后接上，和一直没收一样，撤销、恢复、压缩也照规矩算。
#[test]
fn appended_while_shelved_lands_as_if_never_shelved() {
    let log = busy_log();
    let more = [
        event(22, None, ALICE, "turn.unreverted", r#"{"turns":[18]}"#),
        message(23, ALICE),
    ];
    let mut kept = feed(log.clone());
    let mut shelved = kept.clone();
    shelved.shelve().unwrap();
    let from = shelved.shelved(Seq::new(22).unwrap()).unwrap();
    for event in more.iter().cloned() {
        kept.append(event.clone());
        shelved.append(event);
    }
    assert!(shelved.events().is_empty(), "收着的时候投影看到的是空的");
    shelved.unshelve(&read_back(&log, from)).unwrap();
    assert_eq!(shelved, kept);
}

/// 留着一切的那一份不收：它是读日志的人用的，没有一直留在内存里的。
#[test]
fn the_whole_history_is_not_shelved() {
    let mut whole = History::whole();
    for event in busy_log() {
        whole.append(event);
    }
    assert!(whole.shelve().is_none());
}

/// 改了的文件的原文：日志里有，内核收下时去掉（施工 V-18）；放回来的同样去掉，和收起来以前一样。
#[test]
fn put_back_forgets_file_texts_too() {
    let mut log = two_turns();
    let changed = r#"{"call_id":"call_8_1","status":"ok","blocks":[],"effects":[{"kind":"file.changed","path":"/a","before":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","after":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","before_text":"old","after_text":"new"}]}"#;
    log.push(message(10, ALICE));
    log.push(event(
        11,
        Some(11),
        KERNEL,
        "turn.started",
        r#"{"trigger":10}"#,
    ));
    log.push(event(
        12,
        Some(11),
        MODEL,
        "message.assistant",
        &calls(12, 11, 1),
    ));
    log.push(event(
        13,
        Some(11),
        KERNEL,
        "tool.result",
        &changed.replace("call_8_1", "call_12_1"),
    ));
    let mut history = feed(log.clone());
    let before = history.clone();
    history.shelve().unwrap();
    let from = history.shelved(Seq::new(14).unwrap()).unwrap();
    history.unshelve(&read_back(&log, from)).unwrap();
    assert_eq!(history, before);
    // 从日志的一段重建的（载入、撤掉压缩）：留着一切地收、落到检查点上，原文也去掉，放回的一样。
    let mut rebuilt = History::whole();
    for event in log.iter().cloned() {
        rebuilt.append(event);
    }
    rebuilt.settle();
    assert_eq!(rebuilt, before, "重建的和一路追加的一样");
    rebuilt.shelve().unwrap();
    rebuilt.unshelve(&read_back(&log, from)).unwrap();
    assert_eq!(rebuilt, before);
}
