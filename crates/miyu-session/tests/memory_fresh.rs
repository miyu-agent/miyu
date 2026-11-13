//! 人生会话醒来时交新记的（施工 LF-6，`docs/blueprint/memory.md` 第三条第 9 款）：真会话、真记忆日志，数据根在临时目录。人生
//! 循环叫醒的一轮开头，交这一段上下文里交过以后别处新记的，新的在前、一行一个写法逐字节对，下次醒来没有新的不交；她在这里过的、
//! 合并改出来的、作废的、出处死了的不交；第一轮、压缩以后常驻那一块先交、不重复；人开的、别的循环和定时任务叫醒的、记忆关着的
//! 没有；超过上限截在一条的边界上；记在日志里，重新载入原样回放；读得慢的这一轮不交，下次醒来交。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_kernel::time::Timestamp;
use miyu_policy::memory::MemoryScope;
use miyu_recall::{MemoryEvent, MemoryId, Retired, Saved, Source};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::*;

mod prefix;

fn persona() -> Room {
    Room::persona("engineer")
}

/// 记下的时刻：UTC 的 10 月 7 日 20 点，会话的时区（东九区）里已经是 8 日。
fn saved_at() -> Timestamp {
    Timestamp::parse("2026-10-07T20:00:00.000Z").expect("合写法")
}

/// 一处出处：会话 `session` 的第 1 轮。
fn source(session: &SessionId) -> Source {
    Source {
        session: session.clone(),
        turn: TurnId::new(Seq::new(1).expect("从 1 起")),
    }
}

/// `by` 在这一间里记一条，出处是 `sources`，交回编号。
fn save(
    home: &Home,
    text: &str,
    sources: &[&SessionId],
    by: By,
    replaces: Option<MemoryId>,
) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "episode".into(),
        text: text.into(),
        sources: sources.iter().map(|session| source(session)).collect(),
        audience: vec![alice()],
        replaces,
        about: None,
        venue: None,
        keywords: None,
        who: None,
        people: Vec::new(),
        place: None,
        imported: None,
    };
    log.append(saved_at(), by, None, &MemoryEvent::Saved(saved))
        .expect("记得下")
        .id
}

/// 人在界面上记一条（没有出处）。
fn note(home: &Home, text: &str) -> MemoryId {
    save(home, text, &[], alice(), None)
}

fn retire(home: &Home, id: MemoryId) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let retired = MemoryEvent::Retired(Retired {
        id,
        why: "不对".into(),
        kind: None,
        kept: None,
    });
    log.append(saved_at(), alice(), None, &retired)
        .expect("记得下");
}

/// 一条在块里的样子：和 `memory_search` 的一行一个写法，日期照会话的时区。
fn line(id: MemoryId, text: &str) -> String {
    format!("{id} episode 2026-10-08: {text}")
}

/// 定时表 `t1` 叫醒，循环的种类是 `looping`（没有的是定时任务）。
fn woken_by(looping: Option<&str>) -> By {
    let looping = looping.map_or(String::new(), |looping| format!(r#","loop":"{looping}""#));
    serde_json::from_str(&format!(
        r#"{{"kind":"schedule","schedule":"t1"{looping}}}"#
    ))
    .expect("合写法")
}

fn life() -> By {
    woken_by(Some("life"))
}

/// `by` 交一句、开一轮，等这一轮结束。
async fn turn(handle: &Handle, id_text: &str, by: By, words: &str) {
    let mut pushes = watch(handle).await;
    within(
        &format!("命令 {id_text} 的回应"),
        handle.command(id(id_text), by, say(words)),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

async fn chat(handle: &Handle, id_text: &str, words: &str) {
    turn(handle, id_text, alice(), words).await;
}

async fn wake(handle: &Handle, id_text: &str) {
    turn(handle, id_text, life(), "醒来，过你的日子。").await;
}

/// 一次请求里人那一边的字块，照先后。
fn texts(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text),
            _ => None,
        })
        .collect()
}

/// 请求里外壳是 `<memories>` 的几块（常驻那一块、这种块），照先后。
fn blocks(request: &Request) -> Vec<String> {
    texts(request)
        .into_iter()
        .filter(|text| text.starts_with("<memories>"))
        .collect()
}

/// 会话日志里记忆模块交的 `kind` 那一类的几块：原文和 `refs`，照先后。
fn injected(home: &Home, session: &SessionId, kind: &str) -> Vec<(String, Vec<String>)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == kind => {
                Some((fact.text, fact.refs))
            }
            _ => None,
        })
        .collect()
}

/// 一个别的会话：在跑过、目录在，出处在它里面的算活的。
async fn elsewhere(home: &Home) -> SessionId {
    let script = Script::new([Play::Says("好。")]);
    let other = home.create(&script).await;
    let session = other.id().clone();
    stop(&other).await;
    session
}

#[tokio::test]
async fn a_life_wake_brings_what_was_saved_since_and_only_once() {
    let home = Home::new();
    let old = note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let fixed = save(&home, "下午和主人修好了一个问题", &[&other], alice(), None);
    let pad = save(&home, "群友换了手柄", &[&other], alice(), None);
    wake(&handle, "cmd-2").await;
    let expected = format!(
        "<memories>\n{}\n{}\n</memories>\n",
        line(pad, "群友换了手柄"),
        line(fixed, "下午和主人修好了一个问题")
    );
    let requests = script.requests();
    let summary = format!("<memories>\n{}\n</memories>\n", line(old, "主人养了一只猫"));
    assert_eq!(
        blocks(&requests[1].1),
        [summary.clone(), expected.clone()],
        "常驻那一块原样在前，新记的新的在前"
    );
    let said = texts(&requests[1].1);
    let block = said.iter().position(|text| text == &expected).expect("有");
    let trigger = said
        .iter()
        .position(|text| text.contains("醒来，过你的日子。"))
        .expect("有叫醒那段话");
    assert!(block < trigger, "在叫醒那段话前面：{said:?}");
    assert_eq!(
        injected(&home, handle.id(), "fresh"),
        [(expected.clone(), vec![pad.to_string(), fixed.to_string()])],
        "记成一块 fresh，refs 是列的那几条"
    );

    wake(&handle, "cmd-3").await;
    assert_eq!(
        injected(&home, handle.id(), "fresh").len(),
        1,
        "下一次醒来没有新记的，不交"
    );
    assert_eq!(blocks(&script.requests()[2].1), [summary, expected]);
    stop(&handle).await;
}

#[tokio::test]
async fn what_she_lived_here_merged_dropped_or_dead_is_left_out() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let here = handle.id().clone();
    let other = elsewhere(&home).await;
    let gone = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").expect("合写法");
    let memory: By = serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法");
    save(
        &home,
        "她在人生会话里画了一幅画",
        &[&here],
        memory.clone(),
        None,
    );
    let before = save(&home, "主人在学吉他", &[&other], memory.clone(), None);
    save(
        &home,
        "主人在学吉他和尤克里里",
        &[&other],
        memory,
        Some(before),
    );
    let wrong = save(&home, "主人住在北京", &[&other], alice(), None);
    retire(&home, wrong);
    save(&home, "删掉的会话里说的", &[&gone], alice(), None);
    let both = save(&home, "两边都说过的事", &[&here, &other], alice(), None);
    let noted = note(&home, "人在界面上记的");
    wake(&handle, "cmd-2").await;
    let fresh = injected(&home, handle.id(), "fresh");
    assert_eq!(
        fresh
            .iter()
            .map(|(_, refs)| refs.clone())
            .collect::<Vec<_>>(),
        [vec![noted.to_string(), both.to_string()]],
        "只有不全在这里的、人记的：{fresh:?}"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn the_first_wake_and_after_compaction_the_summary_goes_first_and_is_not_repeated() {
    let home = Home::new();
    let other = elsewhere(&home).await;
    let first = save(&home, "第一件事", &[&other], alice(), None);
    // 这一条断言的是会话的头一轮：判出处活不活要开回合库，头一次开是冷的（建库），慢的机器上会超过回合开始的时限、这一轮
    // 不交（Windows 的 CI 上碰到过一次）。先开好，这里不测冷开。
    let _ = home.recall.turns(&persona());
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("嗯。"),
        Play::Says("<summary>她醒过一次。</summary>"),
        Play::Says("嗯。"),
    ])
    .window(1_000_000);
    let handle = home.create(&script).await;
    wake(&handle, "cmd-1").await;
    assert_eq!(
        blocks(&script.requests()[0].1),
        [format!(
            "<memories>\n{}\n</memories>\n",
            line(first, "第一件事")
        )],
        "第一轮只有常驻那一块"
    );
    // 一句很长的，压得掉。
    chat(&handle, "cmd-2", &"x".repeat(70_000)).await;
    let second = save(&home, "第二件事", &[&other], alice(), None);
    let mut pushes = watch(&handle).await;
    let compact = Command::Compact { instructions: None };
    ask(&handle, "cmd-3", compact).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    wake(&handle, "cmd-4").await;
    let requests = script.requests();
    assert_eq!(
        blocks(&requests[requests.len() - 1].1),
        [format!(
            "<memories>\n{}\n{}\n</memories>\n",
            line(second, "第二件事"),
            line(first, "第一件事")
        )],
        "压缩以后常驻那一块重交，列过的不重复"
    );
    assert!(injected(&home, handle.id(), "fresh").is_empty());
    stop(&handle).await;
}

#[tokio::test]
async fn other_turns_get_no_such_block() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let plays: Vec<Play> = (0..6).map(|_| Play::Says("嗯。")).collect();
    let script = Script::new(plays);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let new = save(&home, "群友换了手柄", &[&other], alice(), None);
    chat(&handle, "cmd-2", "人开的一轮").await;
    turn(&handle, "cmd-3", woken_by(Some("dynamic")), "她自己定的").await;
    turn(&handle, "cmd-4", woken_by(Some("fixed")), "固定间隔").await;
    turn(&handle, "cmd-5", woken_by(None), "定时任务").await;
    assert!(injected(&home, handle.id(), "fresh").is_empty());
    wake(&handle, "cmd-6").await;
    assert_eq!(
        injected(&home, handle.id(), "fresh")
            .into_iter()
            .map(|(_, refs)| refs)
            .collect::<Vec<_>>(),
        [vec![new.to_string()]],
        "同一条，人生循环叫醒的才交"
    );
    stop(&handle).await;

    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let off = Lines {
        memory: MemoryScope::Off,
        ..Lines::default()
    };
    let handle = home
        .create_full(
            &script,
            &miyu_tool::Catalog::default(),
            Opening::default(),
            off,
        )
        .await;
    wake(&handle, "cmd-1").await;
    save(&home, "又一件事", &[&other], alice(), None);
    wake(&handle, "cmd-2").await;
    assert!(
        injected(&home, handle.id(), "fresh").is_empty(),
        "记忆关着的不问"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn a_long_block_is_cut_at_a_line_and_says_how_many_more() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    for n in 0..60 {
        save(
            &home,
            &format!("第 {n} 件事：{}", "一起做过的事".repeat(15)),
            &[&other],
            alice(),
            None,
        );
    }
    wake(&handle, "cmd-2").await;
    let (block, refs) = injected(&home, handle.id(), "fresh").pop().expect("交了");
    assert!(block.len() <= 3000, "上限 3000 字节：{}", block.len());
    let lines: Vec<&str> = block.lines().collect();
    let shown = lines
        .iter()
        .filter(|line| line.contains(" episode "))
        .count();
    assert!(shown > 0 && shown < 60, "{shown}");
    assert_eq!(refs.len(), shown, "refs 只是列出来的");
    assert_eq!(
        lines[lines.len() - 2],
        format!("…and {} more, found by memory_search.", 60 - shown),
        "截在一条的边界上，说还有几条"
    );
    assert!(lines[1].contains("第 59 件事"), "新的在前：{}", lines[1]);
    stop(&handle).await;
}

/// 记在日志里，重放不重算（施工单验收 6）：停下、重新载入以后下一轮请求里那一块逐字节一样，没有再记一块。
#[tokio::test]
async fn a_reloaded_session_sends_the_same_block() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    save(&home, "群友换了手柄", &[&other], alice(), None);
    wake(&handle, "cmd-2").await;
    let before = blocks(&script.requests()[1].1);
    let session = handle.id().clone();
    stop(&handle).await;

    let script = Script::new([Play::Says("好。")]);
    let handle = home.load(&session, &script).await;
    chat(&handle, "cmd-3", "还在吗").await;
    assert_eq!(blocks(&script.requests()[0].1), before, "原样回放");
    assert_eq!(injected(&home, &session, "fresh").len(), 1, "没有重算");
    stop(&handle).await;
}

/// 读得慢的不挡回合（同常驻那一块，`memory.md` 第三条第 4 款）：另一个线程握着记忆日志的锁，这一轮过了时限照常请求、不带这一块；
/// 放开以后下一次醒来再问，交了。时限是几毫秒不测。
#[tokio::test]
async fn a_slow_read_does_not_hold_the_wake() {
    let home = Home::new();
    note(&home, "主人养了一只猫");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let new = save(&home, "群友换了手柄", &[&other], alice(), None);
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let (locked, held) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        log.book(|_| {
            locked.send(()).expect("测试在等");
            assert!(released.recv().is_err(), "测试放开时关了这一头");
        });
    });
    held.recv().expect("握住了锁");
    wake(&handle, "cmd-2").await;
    assert_eq!(
        blocks(&script.requests()[1].1).len(),
        1,
        "过了时限，这一轮不带"
    );
    drop(release);
    holder.join().expect("放开了");
    wake(&handle, "cmd-3").await;
    assert_eq!(
        injected(&home, handle.id(), "fresh")
            .into_iter()
            .map(|(_, refs)| refs)
            .collect::<Vec<_>>(),
        [vec![new.to_string()]],
        "下一次醒来再问，交了"
    );
    stop(&handle).await;
}

/// 下限照常驻那一块列的最新那一条（蓝图第三条第 9 款的已知取舍）：常驻那一块截掉的较老几条醒来时不补交，只交之后新记的。
#[tokio::test]
async fn what_the_summary_cut_is_not_brought_on_waking() {
    let home = Home::new();
    for n in 0..60 {
        note(
            &home,
            &format!("第 {n} 件事：{}", "一起做过的事".repeat(15)),
        );
    }
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let other = elsewhere(&home).await;
    let new = save(&home, "群友换了手柄", &[&other], alice(), None);
    wake(&handle, "cmd-2").await;
    assert_eq!(
        injected(&home, handle.id(), "fresh")
            .into_iter()
            .map(|(_, refs)| refs)
            .collect::<Vec<_>>(),
        [vec![new.to_string()]],
        "只有之后新记的那一条"
    );
    stop(&handle).await;
}
