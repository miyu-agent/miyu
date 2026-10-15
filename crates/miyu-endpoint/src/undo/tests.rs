//! 撤销回应里「撤掉了几次压缩」只数带着撤掉的回合的（施工 6-9），清空另数（施工 6-8 补）。

use miyu_kernel::event::{Body, CompactTrigger, ContextCompacted, Event};
use miyu_kernel::id::{Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::compactions;

/// 回合 `turn` 里的第 `seq` 条压缩，没写原因的（当作到线了）。
fn compaction(seq: u64, turn: u64) -> Event {
    compaction_of(seq, turn, None)
}

/// 同上，原因是 `trigger`。
fn compaction_of(seq: u64, turn: u64, trigger: Option<CompactTrigger>) -> Event {
    let seq = Seq::new(seq).expect("序号合写法");
    Event {
        seq,
        at: Timestamp::from_unix_millis(0).expect("在范围里"),
        turn: Seq::new(turn).map(TurnId::new),
        by: By::Kernel,
        cause: None,
        body: Body::ContextCompacted(ContextCompacted {
            upto: Seq::FIRST,
            summary: "S".to_string(),
            trigger,
            instructions: None,
            notes: String::new(),
            restored: Vec::new(),
            refills: None,
            prepared: false,
        }),
    }
}

#[test]
fn only_compactions_in_the_undone_turns_count() {
    let log = [compaction(4, 3), compaction(6, 5), compaction(7, 5)];
    let turns = |seqs: &[u64]| -> Vec<TurnId> {
        seqs.iter()
            .filter_map(|n| Seq::new(*n).map(TurnId::new))
            .collect()
    };
    assert_eq!(
        compactions(&log, &turns(&[5]), false),
        2,
        "一轮里压过两次的是 2"
    );
    assert_eq!(compactions(&log, &turns(&[3, 5]), false), 3);
    assert_eq!(compactions(&log, &turns(&[8]), false), 0);
}

/// 清空另数（施工 6-8 补）：数压缩的不算它，数清空的只算它；几种压缩都不是清空。
#[test]
fn clears_are_counted_apart_from_compactions() {
    let log = [
        compaction_of(4, 3, Some(CompactTrigger::Manual)),
        compaction_of(6, 5, Some(CompactTrigger::Clear)),
        compaction_of(8, 7, Some(CompactTrigger::Overflow)),
        compaction_of(10, 9, Some(CompactTrigger::Clear)),
        compaction(12, 11),
    ];
    let turns: Vec<TurnId> = [3, 5, 7, 9, 11]
        .iter()
        .filter_map(|n| Seq::new(*n).map(TurnId::new))
        .collect();
    assert_eq!(compactions(&log, &turns, false), 3);
    assert_eq!(compactions(&log, &turns, true), 2);
    assert_eq!(
        compactions(&log, &turns[1..2], false),
        0,
        "只撤了清空那一轮"
    );
}
