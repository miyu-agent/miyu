//! 一个会话的抽取状态（`Extractor`）：闹钟作废、在路上的只有一个、同一段失败三次才放过；一直不闲的会话该不该抽、不等闲的那一次新一轮开始不撤（施工 E-5）。

use super::*;
use crate::memory::extract::ended_only;
use crate::memory::extract::tests::seq;
use miyu_kernel::id::TurnId;
use miyu_kernel::time::Timestamp;

#[test]
fn an_alarm_is_void_once_another_is_set_or_a_turn_starts() {
    let extractor = Extractor::default();
    let first = extractor.arm();
    assert!(extractor.due(first));
    let second = extractor.arm();
    assert!(!extractor.due(first), "上了新的，旧的作废");
    extractor.cancel();
    assert!(!extractor.due(second), "忙起来了");
    let third = extractor.arm();
    assert!(extractor.start(third));
    assert!(!extractor.due(third), "在路上的只有一个");
    assert!(!extractor.start(third));
    assert_eq!(extractor.finish(seq(1), true), 0);
    assert!(extractor.due(third));
    extractor.start(third);
    extractor.abandon();
    assert!(extractor.due(third), "读不成的放下，不算失败");
}

#[test]
fn the_same_slice_failing_three_times_is_let_go() {
    let extractor = Extractor::default();
    let failed = |after: u64| {
        let generation = extractor.arm();
        assert!(extractor.start(generation));
        extractor.finish(seq(after), false)
    };
    assert_eq!(failed(5), 1);
    assert_eq!(failed(5), 2);
    assert_eq!(failed(9), 1, "换了一段重新数");
    assert_eq!(failed(9), 2);
    assert_eq!(failed(9), 3);
    assert_eq!(failed(9), 1, "放过以后重新数");
}

#[test]
fn an_alarm_is_pending_until_it_starts_or_is_let_go() {
    let extractor = Extractor::default();
    assert!(!extractor.pending(), "没上过闹钟");
    let first = extractor.arm();
    assert!(extractor.pending(), "上了，还没响");
    extractor.disarm(first);
    assert!(!extractor.pending(), "响了、不抽，放下了");
    let second = extractor.arm();
    extractor.disarm(first);
    assert!(extractor.pending(), "作废的那一个放下不算数");
    assert!(extractor.start(second));
    assert!(extractor.pending(), "在路上");
    extractor.finish(seq(1), true);
    assert!(!extractor.pending(), "抽完了");
    extractor.arm();
    extractor.cancel();
    assert!(!extractor.pending(), "忙起来撤掉了");
    let third = extractor.arm();
    extractor.start(third);
    extractor.abandon();
    assert!(!extractor.pending(), "读不成的放下了");
}

/// 一直不闲的会话（施工 E-5）：结束了几轮照落了盘的 `turn.ended` 数，满 `max_turns` 轮或者最早那一轮等满 `max_wait` 才该抽；
/// 开始抽一次清零；有一次在路上、已经有一次在等着的不算该抽。
#[test]
fn a_busy_session_is_overdue_after_enough_turns_or_enough_waiting() {
    let hour = std::time::Duration::from_secs(3600);
    let now = |at: &str| Timestamp::parse(at).expect("合写法");
    let extractor = Extractor::default();
    assert!(
        !extractor.overdue(now("2026-10-27T09:00:00.000Z"), 2, hour),
        "一轮都没结束"
    );
    extractor.note_ended(&[ended(4, "2026-10-27T08:00:00.000Z")]);
    assert!(
        !extractor.overdue(now("2026-10-27T08:59:59.999Z"), 3, hour),
        "一轮、差一毫秒满一小时"
    );
    assert!(
        extractor.overdue(now("2026-10-27T09:00:00.000Z"), 3, hour),
        "最早那一轮正好等满一小时"
    );
    extractor.note_ended(&[
        ended(8, "2026-10-27T08:10:00.000Z"),
        ended(12, "2026-10-27T08:20:00.000Z"),
    ]);
    assert!(
        extractor.overdue(now("2026-10-27T08:20:00.000Z"), 3, hour),
        "正好三轮"
    );
    assert!(
        !extractor.overdue(now("2026-10-27T08:20:00.000Z"), 4, hour),
        "三轮不够四轮"
    );
    // 开始抽清零：最早那一轮从下一轮算起。
    let generation = extractor.arm();
    assert!(extractor.start(generation));
    assert!(
        !extractor.overdue(now("2026-10-27T12:00:00.000Z"), 1, hour),
        "在路上"
    );
    extractor.finish(seq(12), true);
    assert!(
        !extractor.overdue(now("2026-10-27T12:00:00.000Z"), 1, hour),
        "清零了"
    );
    extractor.note_ended(&[ended(16, "2026-10-27T11:30:00.000Z")]);
    assert!(
        !extractor.overdue(now("2026-10-27T12:00:00.000Z"), 2, hour),
        "从这一轮算起才半小时"
    );
    assert!(extractor.overdue(now("2026-10-27T12:30:00.000Z"), 2, hour));
}

/// 不等闲的那一次（施工 E-5）：新一轮开始撤闹钟不撤它，读回来照样开得成，不会让一直不闲的会话永远抽不成；在路上的只有一个。
#[test]
fn a_forced_extraction_survives_a_new_turn() {
    let extractor = Extractor::default();
    extractor.note_ended(&[ended(4, "2026-10-27T08:00:00.000Z")]);
    assert!(extractor.force());
    assert!(extractor.pending(), "等着的也不退下");
    assert!(!extractor.force(), "已经有一次在等着");
    assert!(
        !extractor.overdue(
            Timestamp::parse("2026-10-27T10:00:00.000Z").expect("合写法"),
            1,
            std::time::Duration::ZERO
        ),
        "等着的不再算该抽"
    );
    extractor.cancel();
    extractor.arm();
    extractor.cancel();
    assert!(extractor.start_forced(), "读的时候来了新一轮，照样开");
    assert!(!extractor.start_forced(), "一次只开一回");
    assert!(!extractor.force(), "在路上的不再上");
    extractor.finish(seq(4), true);
    assert!(!extractor.pending());
    assert!(
        !extractor.overdue(
            Timestamp::parse("2026-10-27T10:00:00.000Z").expect("合写法"),
            1,
            std::time::Duration::ZERO
        ),
        "开始抽清零了"
    );
    // 闲下来的闹钟先开了：不等闲的那一次开不成，放下。
    assert!(extractor.force());
    let generation = extractor.arm();
    assert!(extractor.start(generation));
    assert!(!extractor.start_forced());
    assert!(extractor.pending(), "闲下来的那一次还在路上");
    extractor.finish(seq(8), true);
    assert!(!extractor.pending(), "不等闲的那一次也放下了");
    // 读不成的放下：不留在等着。
    assert!(extractor.force());
    extractor.unforce();
    assert!(!extractor.pending());
}

/// 一条 `turn.ended`：第 `n` 条，在 `at` 那一刻。
fn ended(n: u64, at: &str) -> Event {
    Event {
        seq: seq(n),
        at: Timestamp::parse(at).expect("合写法"),
        turn: Some(TurnId::new(seq(n - 1))),
        by: miyu_kernel::origin::By::Kernel,
        cause: None,
        body: Body::TurnEnded(miyu_kernel::event::TurnEnded {
            reason: miyu_kernel::event::EndReason::Completed,
        }),
    }
}

/// 不等闲的那一次只取到最后一条 `turn.ended`（施工 E-5）：以后的（正在跑的那一轮、还没开始的人话）不要；一轮都没结束的
/// 什么都不留。
#[test]
fn a_forced_read_stops_at_the_last_ended_turn() {
    let mut events = vec![
        ended(4, "2026-10-27T08:00:00.000Z"),
        ended(8, "2026-10-27T08:01:00.000Z"),
        ended(12, "2026-10-27T08:02:00.000Z"),
    ];
    events[2].body = Body::TurnStarted(miyu_kernel::event::TurnStarted {
        trigger: None,
        triggers: Vec::new(),
        cwd: None,
        dirs: Vec::new(),
    });
    ended_only(&mut events);
    assert_eq!(
        events.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [seq(4), seq(8)]
    );
    let mut none = vec![ended(4, "2026-10-27T08:00:00.000Z")];
    none[0].body = Body::TurnStarted(miyu_kernel::event::TurnStarted {
        trigger: None,
        triggers: Vec::new(),
        cwd: None,
        dirs: Vec::new(),
    });
    ended_only(&mut none);
    assert!(none.is_empty());
}
