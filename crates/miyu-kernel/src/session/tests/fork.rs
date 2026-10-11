//! 分叉（施工 C-8 上，`docs/blueprint/session/fork.md`）：子会话的日志接着分叉点往下数，历史引用父会话到分叉点为止的有效
//! 历史；接着说一句，交给执行器的那次请求里的有效历史和父会话接着说同一句一样（只是序号接着往下数）。造好、载入的一样；
//! 第一条不对、接不上、停在一轮当中的拒；父会话压缩过的照它的检查点算。

use super::load::Logged;
use super::*;
use crate::event::Forked;
use crate::session::fork::Base;
use crate::testkit::CHILD_SESSION;

fn child_id() -> SessionId {
    SessionId::parse(CHILD_SESSION).unwrap()
}

/// 从父会话的第 `upto` 条分叉的 `session.created`。
fn forked_created(upto: u64) -> SessionCreated {
    let mut created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    created.parent = Some(session_id());
    created.depth = Some(1);
    created.forked = Some(Forked {
        from: session_id(),
        upto: seq(upto),
        seal_after: None,
    });
    created
}

/// 父会话说完一轮（「hi」、「好」），交回它的日志。
fn parent() -> Logged {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.say(seen, "好");
    logged
}

/// 从父会话的日志 `base` 分叉，`session.created` 落了盘。
fn fork(base: &[Event]) -> (Session, Vec<Event>) {
    let upto = base.last().unwrap().seq.get();
    let (mut child, actions) = Session::fork(
        child_id(),
        base,
        id(100),
        alice(),
        at(30),
        forked_created(upto),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    let events = appended_events(&actions);
    child.handle(stored(upto + 1));
    (child, events)
}

/// 在 `last` 条之后说一句「再来」，交给执行器的那次请求：`seen`、组装列出来的有效历史。
fn next(session: &mut Session, last: u64, n: u64) -> (Seq, String) {
    session.handle(send(n, "再来"));
    session.handle(stored(last + 2));
    calls(&session.handle(hooks_done(TurnId::new(seq(last + 2)), Vec::new()))).remove(0)
}

#[test]
fn a_fork_starts_after_the_fork_point_and_sees_the_parent_history() {
    let mut parent = parent();
    let upto = parent.last();
    let (mut child, created) = fork(&parent.log);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].seq.get(), upto + 1, "接着分叉点往下数");
    assert!(matches!(created[0].body, Body::SessionCreated(_)));
    let (child_seen, child_history) = next(&mut child, upto + 1, 2);
    let (parent_seen, parent_history) = next(&mut parent.session, upto, 2);
    assert_eq!(
        child_seen.get(),
        parent_seen.get() + 1,
        "子会话多一条 session.created"
    );
    // 底子那一段逐行一样；子会话多一条自己的 session.created；之后接的一样（序号多一）。
    let child: Vec<&str> = child_history.lines().collect();
    let parent: Vec<&str> = parent_history.lines().collect();
    let base = usize::try_from(upto).unwrap();
    assert_eq!(
        child[..base],
        parent[..base],
        "{child_history}\n---\n{parent_history}"
    );
    assert_eq!(child[base], format!("{} session.created", upto + 1));
    let kinds = |lines: &[&str]| -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.split_once(' ')
                    .map_or("", |(_, kind)| kind)
                    .to_string()
            })
            .collect()
    };
    assert_eq!(kinds(&child[base + 1..]), kinds(&parent[base..]));
}

#[test]
fn a_fork_loads_back_the_same() {
    let parent = parent();
    let (child, created) = fork(&parent.log);
    let mut logged = Logged {
        session: child,
        log: created,
    };
    let seen = logged.ask(2, "再来");
    logged.say(seen, "嗯");
    let last = logged.last();
    let (mut loaded, actions) = Session::load_forked(
        child_id(),
        &parent.log,
        logged.log.clone(),
        at(55),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    assert!(
        actions.is_empty(),
        "走完了的，载入时什么都不补：{actions:?}"
    );
    assert!(loaded.history.is_own(&session_id()), "分叉来源算自己的线");
    assert!(loaded.history.is_own(&child_id()));
    assert_eq!(
        next(&mut loaded, last, 3),
        next(&mut logged.session, last, 3),
        "载入的和活着的接着走一样"
    );
}

#[test]
fn a_fork_must_start_right_after_its_fork_point() {
    let parent = parent();
    let upto = parent.last();
    // 分叉点写错了：底子停在 upto，写的却是更早的一条。
    let wrong = Session::fork(
        child_id(),
        &parent.log,
        id(100),
        alice(),
        at(30),
        forked_created(upto - 1),
        policy(),
        environment("~/src/miyu"),
    );
    assert!(
        matches!(wrong, Err(LoadError::Broken(_))),
        "{:?}",
        wrong.err()
    );
    // 没写 forked 的不认。
    let plain: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let unforked = Session::fork(
        child_id(),
        &parent.log,
        id(100),
        alice(),
        at(30),
        plain,
        policy(),
        environment("~/src/miyu"),
    );
    assert!(unforked.is_err());
    // 自己的日志第一条序号对不上分叉点的，载入拒。
    let (_, mut own) = fork(&parent.log);
    own[0].seq = seq(upto + 2);
    let broken = Session::load_forked(
        child_id(),
        &parent.log,
        own,
        at(55),
        policy(),
        environment("~/src/miyu"),
    );
    assert!(matches!(broken, Err(LoadError::Broken(_))));
}

#[test]
fn a_fork_point_inside_a_turn_is_refused() {
    let mut parent = Logged::new();
    parent.ask(1, "hi");
    // 父会话这一轮还没说完：停在一轮当中。
    let inside = Session::fork(
        child_id(),
        &parent.log,
        id(100),
        alice(),
        at(30),
        forked_created(parent.last()),
        policy(),
        environment("~/src/miyu"),
    );
    assert!(matches!(inside, Err(LoadError::Broken(_))));
}

#[test]
fn a_main_session_still_starts_at_one() {
    let logged = Logged::new();
    let mut created = logged.log[0].clone();
    created.seq = seq(3);
    let mut ledger = crate::ledger::Ledger::for_session(child_id());
    assert!(
        ledger.append(&created).is_err(),
        "没分叉的第一条只能是第 1 条"
    );
    let mut forked = crate::ledger::Ledger::for_session(child_id()).starting_at(seq(3));
    assert!(
        forked.append(&created).is_err(),
        "从第 3 条开始的，第一条要写分叉点是第 2 条"
    );
    if let Body::SessionCreated(body) = &mut created.body {
        body.forked = Some(Forked {
            from: session_id(),
            upto: seq(2),
            seal_after: None,
        });
    }
    assert!(forked.append(&created).is_ok());
}

/// 分叉要照的几样（施工 C-8 上）：闲着的分叉点是最后一条；在跑一轮的退到这一轮开始之前；权限照分叉点为止的。
#[test]
fn the_fork_plan_stops_before_a_running_turn() {
    let mut parent = parent();
    let idle = fork_plan(&session_id(), &parent.log).unwrap();
    assert_eq!(idle.upto.get(), parent.last());
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    assert_eq!(idle.permission, created.permission);
    assert_eq!(idle.policy, created.policy);
    let before = parent.last();
    parent.ask(2, "再来");
    let running = fork_plan(&session_id(), &parent.log).unwrap();
    // 「再来」那一条和 turn.started 都在这一轮里：退到它们之前。
    assert_eq!(
        running.upto.get(),
        before + 1,
        "message.user 在 turn.started 前面，算这一轮之前"
    );
    assert!(fork_plan(&session_id(), &[]).is_err());
}

/// 支线能照继承来的旁听消息开一轮（施工 C-8 上，接入QQ 的会话提）：分叉点以前旁听记下、还没当过触发的，子会话的账本照样
/// 认；当过的照样不认。
#[test]
fn a_fork_hears_what_the_parent_overheard() {
    let parent = parent();
    let (child, _) = fork(&parent.log);
    let base = Base::of(
        &parent.log,
        Forked {
            from: session_id(),
            upto: parent.log.last().unwrap().seq,
            seal_after: None,
        },
    )
    .unwrap();
    assert_eq!(
        child.ledger.next_seq(),
        parent.log.last().unwrap().seq.next().next(),
        "子会话的账本接着分叉点，第一条 session.created 已经记了"
    );
    for event in &parent.log {
        assert_eq!(
            child.ledger.overheard(event.seq),
            base.ledger.overheard(event.seq)
        );
        assert_eq!(
            child.ledger.answered(event.seq),
            base.ledger.answered(event.seq)
        );
    }
}
