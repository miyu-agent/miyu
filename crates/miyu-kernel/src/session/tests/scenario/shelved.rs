//! 场景：闲下来收成槽（施工 LW-10 下，`kernel/session.md`「闲下来收成槽」）。同一份剧本交给两个替身：一个照会话 actor 的样子
//! 歇着就收、要看历史以前从「磁盘」读回放回（替身默认就这样，`testkit/shelf.rs`），一个从头到尾不收（[`Stage::hold_history`]，
//! 施工 LW-10 以前的样子）。每一次请求（主请求、摘要、回顾、起标题）照内核的写法逐字节一样，日志一字不差，收的那个真收过。
//!
//! 剧本走到的：多轮带工具、自动压缩前后、手动压缩、撤销又恢复、撤销不恢复、重做、换模型、别的会话发来的话、崩了再载入、
//! 重启接着干。群里旁听的话、桥用 `events.append` 记的收着时不放回（施工 LW-36）：照群聊的样子走一遍，分叉出来的支线也走一遍，
//! 请求、日志照样一样，收着时真的没有放回。

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

/// 收着的会话有计划地停下（施工 PERF-5）：没有回合在进行，「要重启了」不碰历史，一次都不放回，停下以前一直收着。
#[test]
fn a_shelved_session_restarts_without_putting_the_history_back() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.limits(Some(100_000), None);
    stage.overhear("停下以前的一句");
    assert!(stage.shelved(), "歇着就收");
    let unshelvings = stage.unshelvings();
    stage.restarting();
    assert!(stage.shelved(), "停下时一直收着");
    assert_eq!(stage.unshelvings(), unshelvings, "停下时不放回");
}

/// 收着时停下、再起来接着说话（施工 PERF-5）：请求、日志和一直不收的逐字节一样。
#[test]
fn restarting_while_shelved_asks_byte_for_byte_the_same() {
    let (shelving, holding) = both(|stage| {
        after_one_turn(stage);
        stage.limits(Some(100_000), None);
        stage.overhear("停下以前的一句");
        stage.restart();
        stage.overhear("起来以后的一句");
        stage.model([Line::says("又在。")]);
        stage.say("重启以后");
    });
    same(&shelving, &holding);
    assert!(
        shelving.shelvings() >= 2,
        "收的那个只收了 {} 次",
        shelving.shelvings()
    );
}

/// 群聊的样子（施工 LW-36）：歇着以后群里接连有人说话、桥记事件，照其中一条开一轮（带工具、带最末尾那一句），再接着说话，
/// 崩了再载入以后照样有人说话，最后人自己说一句。
fn group(stage: &mut Stage) {
    after_one_turn(stage);
    stage.limits(Some(100_000), None);
    stage.overhear("第一句");
    stage.bridge("ext.onebot.chat.decided");
    let asked = stage.overhear("@她 在吗");
    stage.overhear("第三句");
    stage.model([Line::says("在。")]);
    stage.respond(&[asked], None);
    for k in 0..5 {
        stage.overhear(&format!("又一句 {k}"));
        stage.bridge("ext.onebot.chat.queued");
    }
    let asked = stage.overhear("@她 读一下 a");
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"/a"}"#)]),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::read("/a", "a 的内容")]);
    stage.respond(&[asked], Some("（群里的提示）"));
    stage.overhear("最后一句");
    stage.crash();
    stage.overhear("崩了以后");
    stage.bridge("ext.onebot.chat.decided");
    stage.model([Line::says("还在。")]);
    stage.say("人说一句");
}

/// 群聊的样子：收着时来的旁听的话、桥记的事件不放回，请求、日志和一直不收的逐字节一样。
#[test]
fn overheard_messages_while_shelved_ask_byte_for_byte_the_same() {
    let (shelving, holding) = both(group);
    same(&shelving, &holding);
    assert!(
        shelving.shelvings() >= 4,
        "收的那个只收了 {} 次",
        shelving.shelvings()
    );
}

/// 收着时接连来旁听的话、桥记的事件：一直收着，一次都不放回；照其中一条开一轮才放回一次。
#[test]
fn overheard_messages_do_not_put_the_history_back() {
    let mut stage = compacting(None);
    after_one_turn(&mut stage);
    stage.limits(Some(100_000), None);
    assert!(stage.shelved(), "歇着就收");
    let (shelvings, unshelvings) = (stage.shelvings(), stage.unshelvings());
    stage.overhear("第一句");
    stage.bridge("ext.onebot.chat.decided");
    let asked = stage.overhear("@她 在吗");
    stage.bridge("ext.onebot.chat.queued");
    assert!(stage.shelved(), "一直收着");
    assert_eq!(
        stage.unshelvings(),
        unshelvings,
        "旁听的话、桥记的事件不放回"
    );
    assert_eq!(stage.shelvings(), shelvings, "一直收着，没有放回再收");
    stage.model([Line::says("在。")]);
    stage.respond(&[asked], None);
    assert_eq!(stage.unshelvings(), unshelvings + 1, "开一轮以前放回一次");
}

/// 分叉出来的支线（施工 LW-36）：底子在父会话的日志里，从哪条读起早于自己的第一条。收着时来旁听的话、桥记的事件，并进槽里
/// 的照样连底子一起读回，请求、日志和一直不收的支线一样，收着时没有放回。
#[test]
fn a_forked_line_overhears_while_shelved_the_same() {
    let mut parent = compacting(None);
    parent.model([Line::says("好。")]);
    parent.say("你好");
    parent.overhear("父会话里的一句");
    let fork = |shelving: bool| {
        let mut line = Stage::forked(
            policy,
            environment("~/src/miyu"),
            at(30),
            parent.log(),
            None,
        );
        if !shelving {
            line.hold_history();
        }
        line.model([Line::says("支线在。")]);
        line.say("支线的第一句");
        let unshelvings = line.unshelvings();
        line.overhear("支线里的旁听");
        line.bridge("ext.onebot.chat.decided");
        let asked = line.overhear("@她 支线");
        if shelving {
            assert!(line.shelved(), "支线歇着也收");
            assert_eq!(line.unshelvings(), unshelvings, "支线旁听不放回");
        }
        line.model([Line::says("支线又在。")]);
        line.respond(&[asked], None);
        line
    };
    let (shelving, holding) = (fork(true), fork(false));
    same(&shelving, &holding);
    assert!(
        shelving.shelvings() >= 2,
        "支线只收了 {} 次",
        shelving.shelvings()
    );
    let first = shelving.log().first().expect("有日志").seq;
    assert!(
        shelving.requests().len() == 2 && first.get() > 1,
        "支线自己的第一条是第 {first} 条，底子在父会话的日志里"
    );
}
