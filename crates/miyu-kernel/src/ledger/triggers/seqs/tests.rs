//! 排好、不重的一串序号（施工 LW-37）：顺序、去重、不照先后来的几种插法，和 `BTreeSet` 答得一样、调试输出一样。

use std::collections::BTreeSet;

use super::*;

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

fn of(seqs: &Seqs) -> Vec<u64> {
    seqs.0.iter().map(|seq| seq.get()).collect()
}

#[test]
fn in_order_ones_go_at_the_end_and_are_found() {
    let mut seqs = Seqs::default();
    assert!(!seqs.contains(seq(1)), "空的什么都没有");
    for n in [2, 3, 7, 8] {
        seqs.insert(seq(n));
    }
    assert_eq!(of(&seqs), [2, 3, 7, 8]);
    for (n, has) in [(1, false), (2, true), (5, false), (8, true), (9, false)] {
        assert_eq!(seqs.contains(seq(n)), has, "{n}");
    }
}

#[test]
fn a_seq_is_kept_once() {
    let mut seqs = Seqs::default();
    seqs.extend(&[seq(2), seq(4), seq(6)]);
    // 和最后一个、中间一个、第一个重的。
    for n in [6, 4, 2, 6] {
        seqs.insert(seq(n));
    }
    seqs.extend(&[seq(4), seq(4)]);
    assert_eq!(of(&seqs), [2, 4, 6]);
}

#[test]
fn out_of_order_ones_go_where_they_belong() {
    let mut seqs = Seqs::default();
    seqs.insert(seq(10));
    // 比第一个小、在中间、紧挨着最后一个的前面、紧挨着第一个的后面。
    for n in [3, 6, 9, 4] {
        seqs.insert(seq(n));
    }
    assert_eq!(of(&seqs), [3, 4, 6, 9, 10]);
    seqs.insert(seq(1));
    seqs.insert(seq(11));
    assert_eq!(of(&seqs), [1, 3, 4, 6, 9, 10, 11]);
    assert!(seqs.contains(seq(4)) && !seqs.contains(seq(5)));
}

/// 照一串打乱的、带重的序号记，每一步都和 `BTreeSet` 比：有没有、先后、调试输出。
#[test]
fn it_answers_like_a_btree_set() {
    let mut seqs = Seqs::default();
    let mut set = BTreeSet::new();
    let mut x: u64 = 0x2545_f491;
    let (mut inserted, mut repeated) = (0, 0);
    for step in 0..400 {
        // 大多照先后来，夹着往回跳的和重的。
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let n = if (x >> 33).is_multiple_of(4) {
            (x >> 40) % 300 + 1
        } else {
            step + 1
        };
        let (back, fresh) = (
            seqs.0.last().is_some_and(|last| last.get() > n),
            !set.contains(&seq(n)),
        );
        inserted += usize::from(back && fresh);
        repeated += usize::from(!fresh);
        seqs.insert(seq(n));
        set.insert(seq(n));
        assert_eq!(
            seqs.0,
            set.iter().copied().collect::<Vec<_>>(),
            "第 {step} 步"
        );
    }
    assert!(
        inserted > 10 && repeated > 10,
        "插进中间的 {inserted} 个，重的 {repeated} 个"
    );
    for n in 1..=420 {
        assert_eq!(seqs.contains(seq(n)), set.contains(&seq(n)), "{n}");
    }
    assert_eq!(format!("{seqs:?}"), format!("{set:?}"));
    assert_eq!(format!("{:?}", Seqs::default()), "{}");
}
