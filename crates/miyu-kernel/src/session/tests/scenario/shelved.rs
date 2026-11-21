//! 场景：闲下来收成槽（施工 LW-10 下，`kernel/session.md`「闲下来收成槽」）。同一份剧本交给两个替身：一个照会话 actor 的样子
//! 歇着就收、要看历史以前从「磁盘」读回放回（替身默认就这样，`testkit/shelf.rs`），一个从头到尾不收（[`Stage::hold_history`]，
//! 施工 LW-10 以前的样子）。每一次请求（主请求、摘要、回顾、起标题）照内核的写法逐字节一样，日志一字不差，收的那个真收过。
//!
//! 剧本走到的：多轮带工具、自动压缩前后、手动压缩、撤销又恢复、撤销不恢复、重做、换模型、别的会话发来的话、崩了再载入、
//! 重启接着干。

use super::compaction::{after_one_turn, compacting};
use super::peers::PEER;
use super::*;

/// 两个替身照同一份剧本走完，交回（收的那个、不收的那个）。
fn both(play: impl Fn(&mut Stage)) -> (Stage, Stage) {
    let mut shelving = compacting(None);
    let mut holding = compacting(None);
    holding.hold_history();
    let held = holding.shelvings();
    play(&mut shelving);
    play(&mut holding);
    assert_eq!(holding.shelvings(), held, "不收的那个收过");
    (shelving, holding)
}

/// 两个替身交出的一样：每一次请求照内核的写法、日志的每一行。
fn same(shelving: &Stage, holding: &Stage) {
    let bytes = |stage: &Stage| -> Vec<(u64, Vec<u8>)> {
        stage
            .requests()
            .iter()
            .map(|(seen, request)| (seen.get(), request.canonical_bytes()))
            .collect()
    };
    assert_eq!(bytes(shelving), bytes(holding), "请求不一样");
    let lines = |stage: &Stage| -> Vec<String> { stage.log().iter().map(Event::to_line).collect() };
    assert_eq!(lines(shelving), lines(holding), "日志不一样");
}

/// 多轮带工具、自动压缩、撤销又恢复、撤销不恢复、换模型、别的会话的话。
#[test]
fn a_shelved_session_asks_byte_for_byte_the_same() {
    let (shelving, holding) = both(|stage| {
        after_one_turn(stage);
        stage.model([Line::says("S1"), Line::says("嗯。")]);
        stage.say("再说一句");
        // 压过一次以后窗口放大：后面几轮不再压，剧本只排回复。
        stage.limits(Some(100_000), None);
        stage.model([
            Line::calls("我看看。", &[("read", r#"{"path":"/a"}"#)]),
            Line::says("看完了。"),
        ]);
        stage.tools([Play::read("/a", "a 的内容")]);
        stage.say("读一下 a");
        let last = *stage.turns().last().expect("开过轮");
        stage.revert(last);
        stage.unrevert();
        stage.model([Line::says("收到。")]);
        stage.peer_says(PEER, "迁移写完了。");
        stage.configure("deepseek/deepseek-v4-flash");
        stage.model([Line::says("换了。")]);
        stage.say("换个模型说");
        let last = *stage.turns().last().expect("开过轮");
        stage.revert(last);
        stage.model([Line::says("重来。")]);
        stage.say("撤了以后再说");
    });
    same(&shelving, &holding);
    assert!(
        shelving.shelvings() >= 5,
        "收的那个只收了 {} 次",
        shelving.shelvings()
    );
}

/// 手动压缩、重做、崩了再载入、有计划的重启：收着的时候直接来这几样，和不收的一样。
#[test]
fn manual_compaction_redo_and_reload_while_shelved() {
    let (shelving, holding) = both(|stage| {
        after_one_turn(stage);
        stage.limits(Some(100_000), None);
        stage.model([Line::says("好的。")]);
        stage.say("第二句");
        stage.model([Line::says("摘要")]);
        stage.compact("摘要");
        stage.model([Line::says("重说。")]);
        stage.redo(None);
        stage.crash();
        stage.model([Line::says("还在。")]);
        stage.say("崩了以后");
        stage.restart();
        stage.model([Line::says("又在。")]);
        stage.say("重启以后");
    });
    same(&shelving, &holding);
    assert!(
        shelving.shelvings() >= 4,
        "收的那个只收了 {} 次",
        shelving.shelvings()
    );
}
