//! 压缩以后放掉多出来的容量（施工 V-37 上）：内容、先后和不缩的一样，容量跟着缩到条数的一倍半；空着的不多的不动；留着一切的
//! 那一份落到检查点上时一样缩。

use super::*;

/// 人问一句、一轮回一句，`n` 次：每次占四个序号，从 2 起。交回事件和下一个序号。
fn rounds(n: u64) -> (Vec<Event>, u64) {
    let mut events = vec![created()];
    let mut seq = 2;
    for _ in 0..n {
        events.push(message(seq, ALICE));
        events.extend(turn(seq + 1, seq));
        seq += 4;
    }
    (events, seq)
}

#[test]
fn a_compaction_gives_back_the_room_it_freed() {
    let (mut events, seq) = rounds(700);
    let grown = feed(events.clone());
    assert!(grown.events.capacity() >= 2048, "压缩以前长到两千八百来条");
    // 只留最后一句话和那一轮：替代到它前面。
    events.extend(compaction_turn(seq, seq - 1, seq - 5));
    let history = feed(events);
    assert_eq!(checkpoint(&history), Some(seq + 1));
    assert_eq!(
        seqs(&history),
        [seq - 4, seq - 3, seq - 2, seq - 1, seq, seq + 2]
    );
    assert!(
        history.events.capacity() <= SLACK,
        "容量 {}",
        history.events.capacity()
    );
}

#[test]
fn a_long_tail_keeps_half_again_its_length() {
    let (mut events, seq) = rounds(600);
    // 替代到第 200 句话以前：留下四百句，一千六百来条，容量 4096 里用了不到一半，缩到一倍半。
    let upto = 2 + 4 * 200 - 1;
    events.extend(compaction_turn(seq, seq - 1, upto));
    let history = feed(events);
    let len = history.events.len();
    assert_eq!(len, usize::try_from(seq - upto + 1).unwrap());
    let capacity = history.events.capacity();
    assert!(
        capacity >= len && capacity <= len * 2,
        "{len} 条，容量 {capacity}"
    );
}

#[test]
fn a_little_room_stays() {
    let mut events = two_turns();
    events.extend(compaction_turn(10, 9, 10));
    let mut ledger = Ledger::default();
    let mut history = History::default();
    let mut before = 0;
    for event in events {
        ledger.append(&event).unwrap();
        if matches!(event.body, Body::ContextCompacted(_)) {
            before = history.events.capacity();
        }
        history.append(event);
    }
    assert_eq!(seqs(&history), [12]);
    assert_eq!(history.events.capacity(), before, "空着的不多，不缩");
}

#[test]
fn settling_the_whole_history_gives_back_the_room_too() {
    let (mut events, seq) = rounds(700);
    events.extend(compaction_turn(seq, seq - 1, seq - 5));
    let mut whole = History::whole();
    for event in events {
        whole.append(event);
    }
    assert!(whole.settle());
    assert_eq!(
        seqs(&whole),
        [seq - 4, seq - 3, seq - 2, seq - 1, seq, seq + 2]
    );
    assert!(whole.events.capacity() <= SLACK);
}
