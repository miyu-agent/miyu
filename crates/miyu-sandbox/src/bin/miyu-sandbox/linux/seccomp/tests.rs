//! 过滤的跳法（施工 MB-1）：每一条跳的目标都落在过滤里面，交回的只有放行和 `EACCES` 两种。真装上以后的效果在
//! `tests/linux.rs` 里经助手量。

use super::*;

#[test]
fn every_jump_lands_inside_and_only_two_answers_come_back() {
    let filter = program();
    for (at, op) in filter.iter().enumerate() {
        if op.code == JEQ || op.code == JGE {
            for skip in [op.jt, op.jf] {
                assert!(
                    at + 1 + usize::from(skip) < filter.len(),
                    "第 {at} 条跳出去了"
                );
            }
        }
    }
    let answers: Vec<u32> = filter
        .iter()
        .filter(|op| op.code == RET)
        .map(|op| op.k)
        .collect();
    assert_eq!(answers, [RET_ALLOW, RET_ERRNO | EACCES]);
    assert_eq!(filter.last().map(|op| op.code), Some(RET), "最后一条交回");
}
