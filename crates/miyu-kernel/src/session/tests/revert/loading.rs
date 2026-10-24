//! 一块块载入（施工 V-15，`docs/construction/V-15-重开大会话边读边重放.md`）：压缩以后压到的那一截当场放掉；后面撤掉了压缩、
//! 起点回到放掉的前面的，照整份走；父会话压缩过的分叉照它的检查点算。载入出来的都和一次喂完的一样（`load` 顺带核对）。

use super::super::load::load;
use super::compaction::compacted_once;
use super::*;

/// 一块块载入（施工 V-15）：压缩以后，压到的那一截只记、放掉了；后面撤掉了这次压缩，起点回到放掉的前面，要照整份日志
/// 走 `Session::load`，载入出来的和一次喂完的一样（`load` 顺带核对）。
#[test]
fn a_compaction_undone_later_in_the_log_rebuilds_from_the_whole_log() {
    let mut s = compacted_once();
    s.revert(TurnId::new(seq(14)));
    let log = s.log().to_vec();
    let mut loader = Session::loader(session_id(), None, &log[..1], policy().reports).unwrap();
    for event in &log {
        loader.feed(vec![event.clone()]).unwrap();
    }
    assert!(
        loader.needs_whole(&policy()),
        "压缩撤掉了，起点在放掉的前面"
    );
    let (session, _) = load(log);
    assert!(
        session
            .history
            .events()
            .iter()
            .any(|event| event.seq == seq(3)),
        "撤掉压缩以后有效历史从头算"
    );
}

/// 一块块载入（施工 V-15）：喂过压缩，压到的那一截当场放掉，手上只留压缩替代到的下一条起的；放掉的不再进有效历史，载入出来的
/// 照样一样（`load` 顺带核对）。
#[test]
fn a_fed_compaction_drops_what_it_replaced() {
    let s = compacted_once();
    let log = s.log().to_vec();
    let mut loader = Session::loader(session_id(), None, &log[..1], policy().reports).unwrap();
    for event in &log {
        loader.feed(vec![event.clone()]).unwrap();
    }
    assert!(!loader.needs_whole(&policy()));
    let first = loader.kept.first().map(|event| event.seq);
    assert_eq!(first, Some(seq(14)), "压到 13 号，手上从 14 号起");
    load(log);
}

/// 父会话压缩过，分叉出来的会话载入（施工 V-15 补的测试，C-8 上「父会话压缩过的照它的检查点算」）：有效历史从父会话那次
/// 压缩替代到的下一条起，压缩以前的不在。
#[test]
fn a_fork_of_a_compacted_parent_loads_from_its_checkpoint() {
    let s = compacted_once();
    let base = s.log().to_vec();
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.parent = Some(session_id());
    created.depth = Some(1);
    created.forked = Some(crate::event::Forked {
        from: session_id(),
        upto: base.last().unwrap().seq,
        seal_after: None,
        within: None,
    });
    let child = SessionId::parse(crate::testkit::CHILD_SESSION).unwrap();
    let (_, actions) = Session::fork(
        child.clone(),
        &base,
        id(100),
        alice(),
        at(30),
        created,
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    let own = appended_events(&actions);
    let mut loader = Session::loader(child, Some(base), &own, policy().reports).unwrap();
    loader.feed(own).unwrap();
    let (session, _) = loader
        .finish(at(55), policy(), environment("~/src/miyu"))
        .unwrap();
    let seqs: Vec<u64> = session
        .history
        .events()
        .iter()
        .map(|event| event.seq.get())
        .collect();
    assert!(!seqs.contains(&3), "压缩以前的不在：{seqs:?}");
    assert!(seqs.contains(&18), "压缩以后的在：{seqs:?}");
}
