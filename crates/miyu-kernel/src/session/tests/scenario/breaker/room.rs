//! 场景：压了又满，看压缩腾没腾出地方（`internal/blueprint/compaction.md` 第十条第 5 条，施工 CT-2）。压完第一次说完了的主请求
//! 报的不超过压缩线的八成，算腾出了地方：照常压，连着到了 `refills` 的两倍（出厂 6 次）才停；没腾出的照 `refills` 停（`breaker.rs`
//! 里的 `compacting_again_within_three_turns_counts_up_and_the_third_time_pauses`）。

use super::*;

/// 一轮里压完接着读文件：摘要；压完的那一次请求调 `read`、报 `report`。压缩线 100 的八成是 80：报 20 的腾出了地方，报 110 的没
/// 腾出（本来就过线，下一步又到线）。
fn compact_then_read(report: u64) -> [Line; 2] {
    [
        Line::says("S"),
        Line::calls("", &[("read", "{}")]).reports(report),
    ]
}

/// 腾出了地方的一次：压完那一次报 20，再读一个文件报 1000，下一步又到线。替身的组装不把读回来的字放进请求，读多了由报的数说。
fn roomy_then_read() -> [Line; 3] {
    let [summary, after] = compact_then_read(20);
    [
        summary,
        after,
        Line::calls("", &[("read", "{}")]).reports(1000),
    ]
}

/// 读回来的一个文件。
fn read_back() -> Play {
    Play::Done("abcd".repeat(10))
}

#[test]
fn reading_on_after_compactions_that_made_room_pauses_only_at_twice_the_count() {
    // 像窗口小的模型压完接着读文件：每次都腾出了地方，连着压 6 次不停；第 7 次到了策略的 3 的两倍，写暂停，放不下的不发。
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    stage.model((0..6).flat_map(|_| roomy_then_read()));
    stage.tools((0..12).map(|_| read_back()));
    stage.say("读一读");
    let expected: Vec<Option<u32>> = std::iter::once(None).chain((1..6).map(Some)).collect();
    assert_eq!(refills(&stage), expected);
    assert_eq!(
        pauses(&stage)
            .iter()
            .map(|paused| paused.reason.clone())
            .collect::<Vec<_>>(),
        [PauseReason::TooLarge]
    );
}

#[test]
fn a_compaction_that_made_room_after_two_that_did_not_still_goes_on() {
    // 前两次压完还报 110（过了线的八成），第三次压完报 20：看的是最近这一次，第四次照常压，不停。
    let mut stage = breaking(Some(PAUSE));
    one_turn_then_line(&mut stage);
    let lines: Vec<Line> = [compact_then_read(110), compact_then_read(110)]
        .into_iter()
        .flatten()
        .chain(roomy_then_read())
        .chain([Line::says("S"), Line::says("好。").reports(20)])
        .collect();
    stage.model(lines);
    stage.tools((0..4).map(|_| read_back()));
    stage.say("读一读");
    assert_eq!(refills(&stage), [None, Some(1), Some(2), Some(3)]);
    assert!(pauses(&stage).is_empty());
}
